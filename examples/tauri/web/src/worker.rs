use futures::{SinkExt, StreamExt};
use schema::Message;
use tracing::Level;
use tracing_wasm::WASMLayerConfigBuilder;
use tuber::Tube;
use wasm_bindgen::prelude::*;

#[tuber::worker(name = "tauri")]
pub async fn init_worker(tube: Tube<Message, Message>) {
    tracing_wasm::set_as_global_default_with_config(
        WASMLayerConfigBuilder::new()
            .set_max_level(Level::DEBUG)
            .build(),
    );

    let (mut tx, mut rx) = tube.split();

    async move {
        while let Some(msg) = rx.next().await {
            if let Ok(Message::Ping { data, timestamp }) = msg {
                let now = js_sys::Date::now();
                let duration = now - timestamp;
                tracing::info!(
                    "[web-worker] received PING from web in {}ms ({:.2}MiB/s)",
                    duration,
                    data.len() as f32 / crate::MEGABYTE as f32 / (duration / 1000.0) as f32
                );

                let _ = tx
                    .send(Message::Ping {
                        data,
                        timestamp: now,
                    })
                    .await;
            }
        }

        tracing::info!("[web-worker] lost connection to main",)
    }
    .await;
}
