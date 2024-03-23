use std::{sync::Arc, thread::JoinHandle};

use futures::{Future, StreamExt};
use serde::Deserialize;
use tauri::{
    command,
    ipc::{Channel, InvokeBody, Request, Response},
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};
use tokio::io::{AsyncWriteExt, DuplexStream};
use tokio::sync::Mutex;
use tuber::{util::reader_stream::ReaderStream, Rx, Tube, Tx, Encode, Decode};

#[derive(Deserialize)]
pub struct Config {}

struct State {
    handle: std::sync::Mutex<Option<JoinHandle<()>>>,
    tx: Arc<Mutex<DuplexStream>>,
    rx: Arc<Mutex<Option<ReaderStream<DuplexStream>>>>,
}

#[tracing::instrument(skip(handler))]
pub fn init<R: Runtime, In, Out, Fut>(handler: fn(Tube<In, Out>) -> Fut) -> TauriPlugin<R, Config>
where
    In: Encode + Decode + 'static,
    Out: Encode + Decode + 'static,
    Fut: Future<Output = ()> + 'static,
{
    Builder::<R, Config>::new("tuber")
        .invoke_handler(tauri::generate_handler![tx, rx])
        .setup(move |app, _api| {
            let (tx_a, rx_a) = tokio::io::duplex(tuber::DEFAULT_BUFFER_SIZE);
            let (tx_b, rx_b) = tokio::io::duplex(usize::MAX);

            let handle = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("runtime");

                runtime.block_on(async move {
                    let tube = Tube::new(Tx::<In>::new(tx_b), Rx::<Out>::new(rx_a));
                    handler(tube).await;
                });
            });

            app.manage(State {
                handle: std::sync::Mutex::new(Some(handle)),
                tx: Arc::new(Mutex::new(tx_a)),
                rx: Arc::new(Mutex::new(Some(ReaderStream::new(rx_b).unbounded()))),
            });

            Ok(())
        })
        .on_drop(|app| {
            let state = app.state::<State>();
            match state.handle.lock() {
                Ok(mut handle) => {
                    let _ = handle.take().map(JoinHandle::join);
                }

                Err(e) => {
                    tracing::debug!("Error cleaning up tauri plugin: {:?}", e);
                }
            };
        })
        .build()
}

#[command]
#[tracing::instrument(skip(state, channel))]
async fn tx(state: tauri::State<'_, State>, channel: Channel) -> Result<(), String> {
    let Some(mut reader_stream) = state.rx.lock().await.take() else {
        return Err("Tuber already opened a connection for this tauri app.".to_string());
    };

    while let Some(res) = reader_stream.next().await {
        match res {
            Ok(bytes) => {
                if let Err(e) = channel.send(Response::new(InvokeBody::Raw(bytes.into()))) {
                    tracing::debug!("Error sending message to tauri channel: {:?}", e);
                    break;
                }
            }

            Err(e) => {
                tracing::debug!("Error reading from stream: {:?}", e);
                break;
            }
        }
    }

    *state.rx.lock().await = Some(reader_stream);

    Err("Tuber channel dropped.".to_string())
}

#[command]
#[tracing::instrument(skip(state, data))]
async fn rx<'a>(state: tauri::State<'a, State>, data: Request<'_>) -> Result<Vec<u8>, String> {
    let mut tx = state.tx.lock().await;
    match data.body() {
        InvokeBody::Raw(bytes) => {
            if let Err(e) = tx.write_all(bytes).await {
                let message = format!("Error writing to async write: {:?}", e);
                tracing::debug!("{}", message);
                return Err(message);
            }
        }

        InvokeBody::Json(_) => {
            return Err("Invalid JSON message received over binary tuber channel.".to_string());
        }
    }

    Ok(vec![])
}
