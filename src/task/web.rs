use crate::error::Error;

pub struct Runtime;

impl super::Runtime for Runtime {
    fn shared(
        &self,
        fut: Box<dyn FnOnce() -> futures::prelude::future::BoxFuture<'static, ()> + Send + 'static>,
    ) -> Result<(), Error> {
        wasm_bindgen_futures::spawn_local(fut());

        Ok(())
    }

    fn blocking(&self, _f: Box<(dyn FnOnce() + std::marker::Send + 'static)>) -> Result<(), Error> {
        Err(Error::TaskError(
            "blocking not supported on this runtime".to_string(),
        ))
    }
}
