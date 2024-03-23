use futures::{SinkExt, StreamExt};
use tuber::Tube;
use wasm_bindgen::prelude::*;

use crate::schema::Message;

#[tuber::worker(name = "tuber_simple")]
pub async fn init_worker(tube: Tube<Message, Message>) {
    tracing_wasm::set_as_global_default_with_config(
        tracing_wasm::WASMLayerConfigBuilder::new()
            .set_max_level(tracing::Level::DEBUG)
            .build(),
    );

    let (mut main_tx, mut main_rx) = tube.split();

    async move {
        while let Some(Ok(Message::Ping { data, timestamp })) = main_rx.next().await {
            let now = js_sys::Date::now();
            let duration = now - timestamp;
            tracing::info!(
                "[worker] got PING from main in {}ms ({:.2}MiB/s)",
                duration,
                data.len() as f32 / crate::MEGABYTE as f32 / (duration / 1000.0) as f32
            );
            let _ = main_tx
                .send(Message::Ping {
                    data,
                    timestamp: now,
                })
                .await;
        }
    }
    .await
}
