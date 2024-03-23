use std::time::{SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use schema::Message;
use tuber::Tube;

const MEGABYTE: usize = 1_045_696;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::DEBUG)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("tracing");

    tauri::Builder::default()
        .plugin(tauri_plugin_tuber::init(handler))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tracing::instrument(skip(tube))]
async fn handler(tube: Tube<Message, Message>) {
    let (mut tx, mut rx) = tube.split();

    async move {
        while let Some(Ok(Message::Ping { data, timestamp })) = rx.next().await {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards")
                .as_millis() as f64;
            let duration = now - timestamp;
            tracing::info!(
                "[tauri native] received PING from web in {}ms ({:.2}MiB/s)",
                duration,
                data.len() as f32 / crate::MEGABYTE as f32 / (duration / 1000.0) as f32
            );

            let _ = tx
                .send(schema::Message::Ping {
                    data,
                    timestamp: now,
                })
                .await;
        }
    }
    .await;
}
