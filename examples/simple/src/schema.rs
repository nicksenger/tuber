use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Ping { data: Vec<u8>, timestamp: f64 },
}
