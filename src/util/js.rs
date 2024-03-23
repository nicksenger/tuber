use std::pin::Pin;

use futures::{channel::mpsc::UnboundedReceiver, Future, SinkExt, StreamExt};
use js_sys::{Array, Uint8Array};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use wasm_streams::{readable::ReadableStream, writable::IntoSink, WritableStream};
use web_sys::{MessageEvent, MessagePort};

use super::reader_stream::ReaderStream;

pub unsafe fn from_global<T: JsCast>(name: &str) -> Result<T, JsValue> {
    let val = js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str(name))?;

    Ok(val.dyn_into()?)
}

pub fn string_or_unknown(js_val: impl Into<JsValue>) -> String {
    js_val
        .into()
        .as_string()
        .unwrap_or_else(|| "unknown".to_string())
}

#[tracing::instrument(skip(message_port))]
pub fn stream_from_message_port(message_port: &MessagePort) -> UnboundedReceiver<MessageEvent> {
    let (tx, rx) = futures::channel::mpsc::unbounded();
    let callback = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
        if let Err(e) = tx.unbounded_send(event) {
            tracing::debug!("Error sending MessagePort output: {:?}", e);
        }
    });

    message_port.set_onmessage(Some(callback.as_ref().unchecked_ref()));
    callback.forget();

    rx
}

#[tracing::instrument(skip(readable_stream, message_port))]
pub fn forward_readable_stream_to_message_port(
    readable_stream: web_sys::ReadableStream,
    message_port: &MessagePort,
) -> Result<Pin<Box<dyn Future<Output = Result<(), JsValue>>>>, js_sys::Error> {
    let message_port = message_port.clone();
    let mut stream = ReadableStream::from_raw(readable_stream)
        .try_into_stream()
        .map_err(|(e, _)| e)?;

    Ok(Box::pin(async move {
        let res = Ok(());

        while let Some(item) = stream.next().await {
            match item {
                Ok(val) => {
                    let transfer_list = Array::of1(&val);
                    if let Err(e) =
                        message_port.post_message_with_transferable(&val, &transfer_list)
                    {
                        tracing::debug!("Error posting message to port: {:?}", e);
                    }
                }

                Err(e) => {
                    tracing::debug!("Error reading stream: {:?}", e);
                }
            }
        }

        res
    }))
}

#[tracing::instrument(skip(message_port, writable_stream))]
pub fn forward_message_port_to_writable_stream(
    message_port: &MessagePort,
    writable_stream: web_sys::WritableStream,
) -> Result<Pin<Box<dyn Future<Output = Result<(), JsValue>>>>, js_sys::Error> {
    let mut sink: IntoSink<'_> = WritableStream::from_raw(writable_stream)
        .try_into_sink()
        .map_err(|(e, _)| e)?;

    let mut rx = stream_from_message_port(message_port);

    Ok(Box::pin(async move {
        let res = Ok(());

        while let Some(event) = rx.next().await {
            if let Err(e) = sink.send(event.data()).await {
                tracing::debug!("Error sending message port data to sink: {:?}", e);
            }
        }

        res
    }))
}

#[tracing::instrument(skip(read, message_port))]
pub fn forward_async_read_to_message_port(
    read: impl AsyncRead + std::marker::Unpin + 'static,
    message_port: &MessagePort,
) -> Result<Pin<Box<dyn Future<Output = Result<(), JsValue>>>>, js_sys::Error> {
    let message_port = message_port.clone();
    let mut reader_stream = ReaderStream::new(read);

    Ok(Box::pin(async move {
        while let Some(x) = reader_stream.next().await {
            match x {
                Ok(bytes) => {
                    let val = new_u8ar(&bytes, true);
                    let transfer_list = Array::of1(&val.buffer().into());
                    let _ =
                        message_port.post_message_with_transferable(&val.into(), &transfer_list);
                }

                Err(e) => {
                    tracing::debug!("Error reading from stream: {:?}", e);
                }
            }
        }

        Ok(())
    }))
}

#[tracing::instrument(skip(message_port, write))]
pub fn forward_message_port_to_async_write(
    message_port: &MessagePort,
    mut write: impl AsyncWrite + std::marker::Unpin + 'static,
) -> Result<Pin<Box<dyn Future<Output = Result<(), JsValue>>>>, js_sys::Error> {
    let message_port = message_port.clone();

    let mut rx = stream_from_message_port(&message_port);

    Ok(Box::pin(async move {
        while let Some(event) = rx.next().await {
            match event.data().dyn_into::<Uint8Array>() {
                Ok(u8ar) => {
                    let bytes = u8ar.to_vec();
                    if let Err(e) = write.write_all(&bytes).await {
                        tracing::debug!("Error writing to async write: {:?}", e);
                    }
                }

                Err(e) => {
                    tracing::debug!("Error reading from stream: {:?}", e);
                }
            }
        }

        Ok(())
    }))
}

pub fn bridge_message_port_to_readable_writable(
    message_port: &MessagePort,
    readable_stream: web_sys::ReadableStream,
    writable_stream: web_sys::WritableStream,
) -> Result<Pin<Box<dyn Future<Output = Result<(), JsValue>>>>, js_sys::Error> {
    let message_port = message_port.clone();

    let a = forward_message_port_to_writable_stream(&message_port, writable_stream)?;
    let b = forward_readable_stream_to_message_port(readable_stream, &message_port)?;

    Ok(Box::pin(async move {
        let (a, b) = futures::future::join(a, b).await;

        a.and_then(|_| b)
    }))
}

pub(crate) fn new_u8ar(slice: &[u8], copy: bool) -> Uint8Array {
    if copy {
        Uint8Array::new(&unsafe { Uint8Array::view(&slice) }.into())
    } else {
        unsafe { Uint8Array::view(slice) }
    }
}
