use futures::{SinkExt, StreamExt};
use schema::Message;
use wasm_bindgen::prelude::*;

pub mod schema;
pub mod worker;

const MEGABYTE: usize = 1_045_696;

#[wasm_bindgen]
pub async fn entrypoint() -> Result<(), String> {
    tracing_wasm::set_as_global_default_with_config(
        tracing_wasm::WASMLayerConfigBuilder::new()
            .set_max_level(tracing::Level::DEBUG)
            .build(),
    );

    let (mut tx, mut rx) = tuber::web::spawn_worker::<Message, Message>("tuber_simple")
        .map_err(|e| e.to_string())?
        .split();

    async move {
        let mut start = js_sys::Date::now();
        let _ = tx
            .send(Message::Ping {
                data: vec![0; MEGABYTE * 4],
                timestamp: start,
            })
            .await;

        while let Some(Ok(Message::Ping { data, timestamp })) = rx.next().await {
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

            let _ = tx.send(Message::Ping { data, timestamp: start }).await;
        }

        tracing::warn!("[main thread] worker rx dropped");
    }
    .await;

    Ok(())
}
