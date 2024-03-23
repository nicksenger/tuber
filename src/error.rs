#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0:?}")]
    IoError(#[from] std::io::Error),

    #[error("JS sys error: {0}")]
    JsSysError(String),

    #[error("JS error: {0}")]
    JsError(String),

    #[error("Task error: {0}")]
    TaskError(String),

    #[error("The task was canceled")]
    TaskCanceled(#[from] futures::channel::oneshot::Canceled),

    #[error("Host runtime error: {0:?}")]
    RuntimeError(#[from] HostRuntimeError),

    #[error(
        "Unable to access runtime: this operation must be called from within a runtime context."
    )]
    RuntimeUnavailable,
}

#[derive(Debug, thiserror::Error)]
pub enum HostRuntimeError {
    #[error("Initialization: {0}")]
    Initialization(String),

    #[error("Execution: {0}")]
    Execution(String),
}

#[cfg(feature = "js")]
#[allow(unused)]
use crate::util::js::string_or_unknown;

#[cfg(feature = "worker")]
impl From<wasm_bindgen::JsValue> for Error {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        Error::JsError(string_or_unknown(value))
    }
}

#[cfg(feature = "worker")]
impl From<js_sys::Error> for Error {
    fn from(value: js_sys::Error) -> Self {
        Error::JsError(string_or_unknown(value))
    }
}
