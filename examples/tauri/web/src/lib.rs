use futures::{SinkExt, StreamExt};
use schema::Message;
use tracing::Level;
use tracing_wasm::WASMLayerConfigBuilder;
use tuber::Tube;
use wasm_bindgen::prelude::*;

pub mod worker;

const MEGABYTE: usize = 1_045_696;

#[tuber::tauri_web]
pub async fn entrypoint(tube: Tube<Message, Message>) -> Result<(), String> {
    tracing_wasm::set_as_global_default_with_config(
        WASMLayerConfigBuilder::new()
            .set_max_level(Level::DEBUG)
            .build(),
    );

    let (mut tauri_tx, mut tauri_rx) = tube.split();
    let (mut worker_tx, mut worker_rx) = tuber::web::spawn_worker::<Message, Message>("tauri")
        .map_err(|e| e.to_string())?
        .split();

    let passthrough = async move {
        loop {
            let fut = futures::future::select(
                tauri_rx.next(),
                wasm_timer::Delay::new(std::time::Duration::from_secs(10)),
            );
            match fut.await {
                futures::future::Either::Left((Some(Ok(Message::Ping { data, timestamp })), _)) => {
                    let now = js_sys::Date::now();
                    let duration = now - timestamp;
                    tracing::info!(
                        "[web main] received PING from tauri in {}ms ({}MiB/s)",
                        duration,
                        data.len() as f32 / crate::MEGABYTE as f32 / (duration / 1000.0) as f32
                    );

                    let _ = worker_tx
                        .send(Message::Ping {
                            data,
                            timestamp: now,
                        })
                        .await;
                }

                futures::future::Either::Left(err) => {
                    tracing::debug!("[web main] unexpected message from tauri: {:?}", err);
                }

                _ => {
                    let Ok((_, rx)) =
                        tuber::tauri::connect().map(|tube: Tube<Message, Message>| tube.split())
                    else {
                        tracing::debug!("[web main] lost connection to tauri");
                        break;
                    };

                    tauri_rx = rx;
                }
            }
        }
    };

    let act = async move {
        let mut start = js_sys::Date::now();
        let _ = tauri_tx
            .send(Message::Ping {
                data: vec![0; MEGABYTE * 4],
                timestamp: start,
            })
            .await;

        while let Some(Ok(Message::Ping { data, timestamp })) = worker_rx.next().await {
            let now = js_sys::Date::now();
            let hop_duration = now - timestamp;
            let roundtrip_duration = now - start;

            tracing::info!(
                "[web main] received PING from worker in {}ms ({:.2}MiB/s), roundtrip time: {}ms ({:.2}MiB/s)",
                hop_duration,
                data.len() as f32 / MEGABYTE as f32 / (hop_duration / 1000.0) as f32,
                roundtrip_duration,
                data.len() as f32 / MEGABYTE as f32 / (roundtrip_duration / 1000.0) as f32,
            );

            let _ = wasm_timer::Delay::new(std::time::Duration::from_secs(1)).await;
            start = js_sys::Date::now();

            let _ = tauri_tx
                .send(Message::Ping {
                    data,
                    timestamp: start,
                })
                .await;
        }
    };

    let _ = futures::join!(passthrough, act);

    Ok(())
}
