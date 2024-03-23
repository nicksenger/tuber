use std::{
    pin::Pin,
    task::{Context, Poll},
};

use futures::{channel::oneshot, future::BoxFuture, Future};

use crate::{error::HostRuntimeError, Error};

#[cfg(feature = "tokio")]
mod tokio_rt;
#[cfg(feature = "wasmer-js")]
mod wasmer_js;
#[cfg(feature = "web")]
mod web;

trait Runtime {
    fn shared(
        &self,
        f: Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send + 'static>,
    ) -> Result<(), Error>;
    fn blocking(&self, f: Box<(dyn FnOnce() + std::marker::Send + 'static)>)
        -> Result<(), Error>;
}

pub fn initialize() -> Result<(), Error> {
    #[cfg(feature = "wasmer-js")]
    {
        wasmer_js::Runtime::lazily_initialized().map_err(|_| {
            Error::RuntimeError(HostRuntimeError::Initialization(
                "Runtime initialization failed".to_string(),
            ))
        })?;
    }

    #[cfg(feature = "tokio")]
    {
        // TODO
    }

    Ok(())
}

/// Spawns a new asyncronous task, returning a [`JoinHandle`] for it.
///
/// This task should not perform blocking work.
#[tracing::instrument(skip(future))]
pub fn spawn<F>(future: F) -> Result<JoinHandle<F::Output>, Error>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    with_task_manager(move |rt| {
        let (tx, rx) = oneshot::channel();

        if let Err(e) = rt.shared(Box::new(|| {
            Box::pin(async move {
                let res = future.await;
                if let Err(_) = tx.send(Ok(res)) {
                    tracing::debug!("Error sending task output");
                }
            })
        })) {
            let (tx, rx) = oneshot::channel();
            if let Err(_) = tx.send(Err(Error::TaskError(e.to_string()))) {
                tracing::debug!("Error sending task error");
            }

            return JoinHandle { rx };
        }

        JoinHandle { rx }
    })
}

/// Spawns a new asyncronous task on the current thread, returning a [`JoinHandle`] for it.
///
/// This task should not perform blocking work.
#[tracing::instrument(skip(future))]
pub fn spawn_local<F>(future: F) -> Result<JoinHandle<F::Output>, Error>
where
    F: Future + 'static,
    F::Output: Send + 'static,
{
    let (tx, rx) = oneshot::channel();

    #[cfg(feature = "web")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            let res = future.await;
            if let Err(_) = tx.send(Ok(res)) {
                tracing::debug!("Error sending task output");
            }
        });

        return Ok(JoinHandle { rx });
    }

    #[cfg(feature = "tokio")]
    {
        tokio::task::spawn_local(async move {
            let res = future.await;
            if let Err(_) = tx.send(Ok(res)) {
                tracing::debug!("Error sending task output");
            }
        });

        return Ok(JoinHandle { rx });
    }

    #[allow(unreachable_code)]
    Err(Error::TaskError(format!("spawn_local not supported")))
}

// Runs the provided closure on a worker where blocking is acceptable.
#[tracing::instrument(skip(f))]
pub fn spawn_blocking<F, R>(f: F) -> Result<JoinHandle<R>, Error>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    with_task_manager(move |rt| {
        let (tx, rx) = oneshot::channel();

        if let Err(e) = rt.blocking(Box::new(move || {
            let res = f();
            if let Err(_) = tx.send(Ok(res)) {
                tracing::debug!("Error sending task output");
            }
        })) {
            let (tx, rx) = oneshot::channel();
            if let Err(_) = tx.send(Err(Error::TaskError(e.to_string()))) {
                tracing::debug!("Error sending task error");
            }

            return JoinHandle { rx };
        }

        JoinHandle { rx }
    })
}

fn with_task_manager<T>(f: impl FnOnce(&dyn Runtime) -> T) -> Result<T, Error> {
    #[cfg(feature = "wasmer-js")]
    return match wasmer_js::Runtime::lazily_initialized() {
        Ok(runtime) => Ok(f(&runtime)),
        Err(_) => Err(Error::RuntimeUnavailable),
    };

    #[cfg(feature = "web")]
    return Ok(f(&web::Runtime));

    #[cfg(feature = "tokio")]
    return tokio::runtime::Handle::try_current()
        .map_err(|e| Error::RuntimeError(HostRuntimeError::Initialization(e.to_string())))
        .map(|handle| f(&handle));
}

pub struct JoinHandle<T> {
    rx: oneshot::Receiver<Result<T, Error>>,
}

impl<T> Future for JoinHandle<T> {
    type Output = Result<T, Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.rx).poll(cx).map(|res| match res {
            Ok(x) => x,
            Err(e) => Err(e.into()),
        })
    }
}
