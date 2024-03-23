#[cfg(feature = "core")]
pub(crate) mod core;
#[cfg(feature = "core")]
mod error;
pub mod serde;
pub use serde::{codec, Decode, Encode};
#[cfg(feature = "macros")]
pub use tuber_macros::*;
#[cfg(feature = "runtime")]
pub mod task;
#[cfg(feature = "runtime")]
pub use task::{spawn, JoinHandle};
#[cfg(feature = "tauri")]
pub mod tauri;
pub mod util;
#[cfg(feature = "web")]
pub mod web;
#[cfg(feature = "worker")]
pub mod worker;
#[cfg(feature = "core")]
pub use core::{Rx, Tube, Tx};
#[cfg(feature = "core")]
pub use error::Error;

pub const DEFAULT_BUFFER_SIZE: usize = 65_536;
