use bytes::buf::Writer;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_core::stream::Stream;
use pin_project_lite::pin_project;
use std::io::copy;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncRead;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error: {0:?}")]
    IoError(#[from] std::io::Error),

    #[error("Zero bytes read from underlying reader")]
    ZeroBytes,
}

pin_project! {
    #[derive(Debug)]
    pub struct ReaderStream<R> {
        #[pin]
        reader: Option<R>,
        buf: BytesMut,
        chunk: Writer<BytesMut>,
        capacity: usize,
        unbounded: bool,
    }
}

impl<R: AsyncRead> ReaderStream<R> {
    pub fn new(reader: R) -> Self {
        ReaderStream {
            reader: Some(reader),
            buf: BytesMut::new(),
            chunk: BytesMut::with_capacity(crate::DEFAULT_BUFFER_SIZE).writer(),
            capacity: crate::DEFAULT_BUFFER_SIZE,
            unbounded: true,
        }
    }

    #[allow(unused)]
    pub fn with_capacity(reader: R, capacity: usize) -> Self {
        ReaderStream {
            reader: Some(reader),
            buf: BytesMut::with_capacity(capacity),
            chunk: BytesMut::new().writer(),
            capacity,
            unbounded: false,
        }
    }

    /// Allows this stream's internal buffer to grow as needed
    /// to accommodate throughput
    #[allow(unused)]
    pub fn unbounded(self) -> Self {
        Self {
            unbounded: true,
            ..self
        }
    }
}

impl<R: AsyncRead> Stream for ReaderStream<R> {
    type Item = std::io::Result<Bytes>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        use tokio_util::io::poll_read_buf;

        loop {
            let mut this = self.as_mut().project();

            let reader = match this.reader.as_pin_mut() {
                Some(r) => r,
                None => return Poll::Ready(None),
            };

            if this.buf.capacity() == 0 {
                this.buf.reserve(*this.capacity);
            }

            let res = match poll_read_buf(reader, cx, &mut this.buf) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
                Poll::Ready(Ok(n)) if *this.unbounded => {
                    let mut reader = this.buf.split().reader();
                    if let Err(e) = copy(&mut reader, &mut this.chunk) {
                        return Poll::Ready(Some(Err(e)));
                    };

                    if n >= *this.capacity {
                        continue;
                    }

                    let chunk = std::mem::replace(
                        this.chunk,
                        BytesMut::with_capacity(*this.capacity).writer(),
                    )
                    .into_inner()
                    .freeze();

                    Poll::Ready(Some(Ok(chunk)))
                }
                Poll::Ready(Ok(_)) => {
                    let chunk = this.buf.split();
                    Poll::Ready(Some(Ok(chunk.freeze())))
                }
            };

            return res;
        }
    }
}
