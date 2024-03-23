use std::sync::Arc;
use wasmer_wasix::{Runtime as Rt, VirtualTaskManager};
use crate::error::Error;

pub use wasmer_js::runtime::Runtime;

impl super::Runtime for Arc<wasmer_js::runtime::Runtime> {
    fn shared(
        &self,
        fut: Box<dyn FnOnce() -> futures::prelude::future::BoxFuture<'static, ()> + Send + 'static>,
    ) -> Result<(), Error> {
        self.task_manager()
            .task_shared(fut)
            .map_err(|e| Error::TaskError(e.to_string()))
    }

    fn blocking(&self, f: Box<(dyn FnOnce() + std::marker::Send + 'static)>) -> Result<(), Error> {
        self.task_manager()
            .task_dedicated(f)
            .map_err(|e| Error::TaskError(e.to_string()))
    }
}
