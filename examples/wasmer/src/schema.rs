use bitcode::{Encode, Decode};

#[derive(Debug, Clone, Encode, Decode)]
pub enum Message {
    Ping { data: Vec<u8>, timestamp: f64 },
}
