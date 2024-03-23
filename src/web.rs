use js_sys::{Array, Uint8Array};
use once_cell::sync::Lazy;
use wasm_bindgen::JsValue;
use web_sys::WorkerType;

use crate::{util::js::string_or_unknown, Decode, Encode, Error, Tube};

static WORKER_URL: Lazy<String> = Lazy::new(|| {
    let script = include_str!("./js/worker.js");

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(
        Array::from_iter([Uint8Array::from(script.as_bytes())]).as_ref(),
        web_sys::BlobPropertyBag::new().type_("application/javascript"),
    )
    .unwrap();

    web_sys::Url::create_object_url_with_blob(&blob).unwrap()
});

/// Spawn the tuber web-worker with the specified name, returning a communication channel
pub fn spawn_worker<In, Out>(name: &str) -> Result<Tube<In, Out>, Error>
where
    In: Encode + Decode,
    Out: Encode + Decode + 'static,
{
    let worker = web_sys::Worker::new_with_options(
        &WORKER_URL,
        web_sys::WorkerOptions::new()
            .name(name)
            .type_(WorkerType::Module),
    )
    .map_err(|e| Error::JsError(string_or_unknown(e)))?;

    let channel =
        web_sys::MessageChannel::new().map_err(|e| Error::JsError(string_or_unknown(e)))?;
    let (host_port, worker_port) = (channel.port1(), channel.port2());

    let origin = web_sys::window()
        .ok_or_else(|| Error::JsError("Failed to acquire window object".to_string()))?
        .origin();

    let message = Array::of3(
        &worker_port,
        &JsValue::from_str(name),
        &JsValue::from_str(&origin),
    );
    let transfer_list = Array::of1(&worker_port);
    worker
        .post_message_with_transfer(&message, &transfer_list)
        .map_err(|e| Error::JsError(string_or_unknown(e)))?;

    let tube = Tube::try_from_message_port_spawned(&host_port)?;

    Ok(tube)
}
