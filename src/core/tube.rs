use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use futures::{Future, Sink, SinkExt, Stream, StreamExt};
use js_sys::Uint8Array;
use pin_project_lite::pin_project;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serde::SymmetricallyFramed;
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};
use tokio_util::io::StreamReader;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{MessagePort, ReadableStream, WritableStream};

use crate::codec::SymmetricalTuber;
use crate::{Decode, Encode};
use crate::util::js::{
    forward_async_read_to_message_port, forward_message_port_to_async_write, new_u8ar,
    string_or_unknown,
};
use crate::util::reader_stream::ReaderStream;
use crate::Error;
use crate::DEFAULT_BUFFER_SIZE;

pin_project! {
/// A handle used for communication between host and workers
pub struct Tube<In, Out> {
    #[pin]
    tx: Tx<In>,
    #[pin]
    rx: Rx<Out>,
}
}

impl<In, Out> Tube<In, Out>
where
    In: Encode + Decode,
    Out: Encode + Decode,
{
    pub fn new(tx: Tx<In>, rx: Rx<Out>) -> Self {
        Self { tx, rx }
    }

    /// Split the tube into its underlying sink & stream
    pub fn split(self) -> (Tx<In>, Rx<Out>) {
        (self.tx, self.rx)
    }

    /// Attempts to convert a message port into a tube, returning both the tube
    /// as well as a future used to drive its processing to and from the
    /// underlying message port.
    pub fn try_from_message_port(
        message_port: &MessagePort,
    ) -> Result<(Self, Pin<Box<dyn Future<Output = Result<(), Error>>>>), Error> {
        let (tx_write, tx_read) = tokio::io::duplex(DEFAULT_BUFFER_SIZE);
        let (rx_write, rx_read) = tokio::io::duplex(DEFAULT_BUFFER_SIZE);

        let tube = Self::new(Tx::new(tx_write), Rx::new(rx_read));

        let read_fut = forward_async_read_to_message_port(tx_read, message_port)
            .map_err(|e| Error::JsSysError(string_or_unknown(e.as_string())))?;
        let write_fut = forward_message_port_to_async_write(message_port, rx_write)
            .map_err(|e| Error::JsSysError(string_or_unknown(e.as_string())))?;

        let fut = async move {
            let (a, b) = futures::future::join(read_fut, write_fut).await;

            a.and_then(|_| b)
                .map_err(|e| Error::JsError(string_or_unknown(e)))
        };

        Ok((tube, Box::pin(fut)))
    }

    /// Attempts to convert a message port into a tube, spawning the future used to drive its
    /// processing onto the JS event loop & ignoring any errors.
    #[tracing::instrument(skip(message_port))]
    pub fn try_from_message_port_spawned(message_port: &MessagePort) -> Result<Self, Error> {
        let (tube, fut) = Self::try_from_message_port(message_port)?;

        #[cfg(feature = "runtime")]
        crate::task::spawn_local(async move {
            if let Err(e) = fut.await {
                tracing::debug!("Error spawning message port future: {:?}", e);
            }
        })?;

        #[cfg(not(feature = "runtime"))]
        return Err(Error::TaskError(
            "Attempted to spawn a future, but there is no executor available",
        ));

        Ok(tube)
    }
}

impl<In, Out> Stream for Tube<In, Out>
where
    Out: Decode,
{
    type Item = Result<Out, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project().rx.as_mut().poll_next(cx).map_err(Into::into)
    }
}

impl<In, Out> Sink<In> for Tube<In, Out>
where
    In: Encode,
{
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .tx
            .as_mut()
            .poll_ready(cx)
            .map_err(Into::into)
    }

    fn start_send(self: Pin<&mut Self>, item: In) -> Result<(), Self::Error> {
        self.project()
            .tx
            .as_mut()
            .start_send(item)
            .map_err(Into::into)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .tx
            .as_mut()
            .poll_flush(cx)
            .map_err(Into::into)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .tx
            .as_mut()
            .poll_close(cx)
            .map_err(Into::into)
    }
}

type Front<T> = SymmetricallyFramed<
    FramedWrite<Pin<Box<dyn AsyncWrite>>, LengthDelimitedCodec>,
    T,
    SymmetricalTuber<T>,
>;

pin_project! {
    /// Sink associated with a `Tube`
    pub struct Tx<T> {
        #[pin]
        inner: Front<T>
    }
}

impl<T> Tx<T>
where
    T: Encode + Decode,
{
    pub fn new(tx: impl AsyncWrite + 'static) -> Self {
        Self {
            inner: SymmetricallyFramed::new(
                FramedWrite::new(Box::pin(tx), LengthDelimitedCodec::new()),
                SymmetricalTuber::default(),
            ),
        }
    }

    pub fn into_writer(self) -> impl AsyncWrite {
        self.inner.into_inner().into_inner()
    }

    #[tracing::instrument(skip(chunked_sink))]
    pub fn from_chunked_sink<E>(
        chunked_sink: impl Sink<Bytes, Error = E> + 'static,
    ) -> (Self, Pin<Box<dyn Future<Output = Result<(), Error>>>>)
    where
        E: Into<std::io::Error> + 'static,
    {
        let (tx, rx) = tokio::io::duplex(DEFAULT_BUFFER_SIZE);
        let reader_stream = ReaderStream::new(rx).filter_map(|x| {
            futures::future::ready(
                x.map(|x| Ok(x))
                    .map_err(|e| {
                        tracing::debug!("Error reading stream: {:?}", e);
                        e
                    })
                    .ok(),
            )
        });
        let fut = async move {
            reader_stream
                .forward(chunked_sink)
                .await
                .map_err(Into::into)?;

            Ok(())
        };

        (Self::new(tx), Box::pin(fut))
    }

    #[tracing::instrument(skip(chunked_sink))]
    pub fn from_chunked_sink_spawned<E>(
        chunked_sink: impl Sink<Bytes, Error = E> + 'static,
    ) -> Result<Self, Error>
    where
        E: Into<std::io::Error> + 'static,
    {
        let (tx, fut) = Self::from_chunked_sink(chunked_sink);

        #[cfg(feature = "runtime")]
        crate::task::spawn_local(fut)?;

        #[cfg(not(feature = "runtime"))]
        return Err(Error::TaskError(
            "Attempted to spawn a future, but there is no executor available",
        ));

        Ok(tx)
    }

    pub fn from_writable_stream(
        writable_stream: WritableStream,
    ) -> (Self, Pin<Box<dyn Future<Output = Result<(), Error>>>>) {
        let sink = wasm_streams::WritableStream::from_raw(writable_stream)
            .into_sink()
            .with(move |bytes: Bytes| {
                let arr = new_u8ar(&bytes, true);
                futures::future::ready(Ok::<JsValue, JsValue>(arr.into()))
            })
            .sink_map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, string_or_unknown(e)));

        Self::from_chunked_sink(sink)
    }

    #[tracing::instrument(skip(writable_stream))]
    pub fn from_writable_stream_spawned(writable_stream: WritableStream) -> Result<Self, Error> {
        let (tx, fut) = Self::from_writable_stream(writable_stream);

        #[cfg(feature = "runtime")]
        crate::task::spawn_local(fut)?;

        #[cfg(not(feature = "runtime"))]
        return Err(Error::TaskError(
            "Attempted to spawn a future, but there is no executor available",
        ));

        Ok(tx)
    }
}

impl<T> Sink<T> for Tx<T>
where
    T: Encode,
{
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .as_mut()
            .poll_ready(cx)
            .map_err(Into::into)
    }

    fn start_send(self: Pin<&mut Self>, item: T) -> Result<(), Self::Error> {
        self.project()
            .inner
            .as_mut()
            .start_send(item)
            .map_err(Into::into)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .as_mut()
            .poll_flush(cx)
            .map_err(Into::into)
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.project()
            .inner
            .as_mut()
            .poll_close(cx)
            .map_err(Into::into)
    }
}

type Back<T> = SymmetricallyFramed<
    FramedRead<Pin<Box<dyn AsyncRead>>, LengthDelimitedCodec>,
    T,
    SymmetricalTuber<T>,
>;

pin_project! {
    /// Stream associated with a `Tube`
    pub struct Rx<T> {
        #[pin]
        inner: Back<T>
    }
}

impl<T> Rx<T>
where
    T: Encode + Decode,
{
    pub fn new(rx: impl AsyncRead + 'static) -> Self {
        Self {
            inner: SymmetricallyFramed::new(
                FramedRead::new(Box::pin(rx), LengthDelimitedCodec::new()),
                SymmetricalTuber::default(),
            ),
        }
    }

    pub fn from_chunked_stream<B, E>(
        chunked_stream: impl Stream<Item = Result<B, E>> + 'static,
    ) -> Self
    where
        B: Buf + 'static,
        E: Into<std::io::Error>,
    {
        let stream_reader = StreamReader::new(chunked_stream);

        Self::new(stream_reader)
    }

    pub fn from_readable_stream(readable_stream: ReadableStream) -> Self {
        let chunked_stream = wasm_streams::ReadableStream::from_raw(readable_stream)
            .into_stream()
            .map(
                |res| match res.and_then(|val| val.dyn_into::<Uint8Array>()) {
                    Ok(u8ar) => Ok(Bytes::from(u8ar.to_vec())),

                    Err(e) => Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        string_or_unknown(e),
                    )),
                },
            );

        Self::from_chunked_stream(chunked_stream)
    }

    /// Returns a future which drives copying of the underlying reader into an async writer
    pub fn pipe(self, tx: Tx<T>) -> impl Future<Output = tokio::io::Result<()>> {
        self.pipe_to_writer(tx.inner.into_inner().into_inner())
    }

    /// Returns a future which drives copying of the underlying reader into an async writer
    pub fn pipe_to_writer(
        self,
        mut sink: impl AsyncWrite + Unpin,
    ) -> impl Future<Output = tokio::io::Result<()>> {
        async move {
            tokio::io::copy(&mut self.inner.into_inner().into_inner(), &mut sink).await?;

            Ok(())
        }
    }
}

impl<T> Stream for Rx<T>
where
    T: Decode,
{
    type Item = Result<T, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.project()
            .inner
            .as_mut()
            .poll_next(cx)
            .map_err(Into::into)
    }
}
