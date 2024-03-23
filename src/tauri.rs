use wasm_bindgen::prelude::*;

use crate::{util::js::string_or_unknown, Error, Tube, Decode, Encode};

#[wasm_bindgen(module = "/src/js/tauri.js")]
extern "C" {
    fn __tuber_tauri() -> JsValue;
}

#[tracing::instrument]
pub fn connect<In, Out>() -> Result<Tube<In, Out>, Error>
where
    In: Encode + Decode + 'static,
    Out: Encode + Decode + 'static,
{
    let message_port = __tuber_tauri()
        .dyn_into()
        .map_err(|e| Error::JsError(string_or_unknown(e)))?;
    let (tube, fut) = Tube::try_from_message_port(&message_port)?;

    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = fut.await {
            tracing::debug!("Error running tauri web future: {:?}", e);
        }
    });

    Ok(tube)
}
