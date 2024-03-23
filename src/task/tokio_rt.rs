use tokio::runtime::Handle;

use crate::error::Error;

impl super::Runtime for Handle {
    fn blocking(&self, f: Box<(dyn FnOnce() + std::marker::Send + 'static)>) -> Result<(), Error> {
        let _ = tokio::task::spawn(async move {
            let _ = tokio::task::spawn_blocking(f).await;
        });

        Ok(())
    }

    fn shared(
        &self,
        f: Box<dyn FnOnce() -> futures::prelude::future::BoxFuture<'static, ()> + Send + 'static>,
    ) -> Result<(), Error> {
        let _ = tokio::task::spawn(f());

        Ok(())
    }
}
