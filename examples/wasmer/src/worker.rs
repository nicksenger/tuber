use futures::{SinkExt, StreamExt};
use tuber::Tube;
use wasm_bindgen::prelude::*;

use crate::schema::Message;

#[tuber::worker(name = "tuber_wasmer")]
pub async fn init_worker(tube: Tube<Message, Message>) {
    wasmer_js::initialize_logger(Some("debug".to_string())).expect("logger");

    let (mut main_tx, mut main_rx) = tube.split();

    let handle = tuber::task::spawn_blocking(|| {
        let mut n: i32 = 0;
        for m in 0..1_000_000 {
            n = n.wrapping_add(m);
        }

        n
    }).expect("handle");

    tuber::spawn(async move {
        let n = handle.await.expect("result");
        tracing::info!("[worker] blocking work from another thread finished up: {}", n);
    });

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
    }.await
}
