use std::{future::Future, pin::Pin, task::{Context, Poll}, time::Duration};

use futures_util::{stream::{self, Stream, StreamExt}};

use log::error;
use stream_throttle::{ThrottlePool, ThrottleRate, ThrottledStream};
use tokio::sync::{mpsc, OnceCell};

type Message = Box<dyn super::message::Message + Send + Sync>;

pub static TX: OnceCell<mpsc::UnboundedSender<Box<dyn Stream<Item=Message> + Send + Unpin>>> = OnceCell::const_new();

pub async fn add<S>(stream: S) -> Result<(), ()>
where S: Stream<Item=Message> + Send + Unpin + 'static {
    TX.get_or_init(|| async {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let stream = SelectAll::new(rx);
            // we can send globally only 30 telegram messages per second
            let rate = ThrottleRate::new(30, Duration::from_secs(1));
            let pool = ThrottlePool::new(rate);
            stream.throttle(pool).for_each_concurrent(None, |_m| async {}).await;
        });
        tx
    }).await
        .send(Box::new(stream))
        .map_err(|e| error!("Stream error: {}", e))
}

pub struct SelectAll<S> {
    rx: mpsc::UnboundedReceiver<S>,
    inner: stream::SelectAll<S>,
}

impl<S> SelectAll<S>
where S: Stream<Item=Message> + Unpin {
    fn new(rx: mpsc::UnboundedReceiver<S>) -> Self {
        SelectAll {
            rx,
            inner: stream::SelectAll::new(),
        }
    }
}

impl<S> Stream for SelectAll<S>
where S: Stream + Unpin {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        while let Poll::Ready(Some(s)) = Box::pin(this.rx.recv()).as_mut().poll(cx) {
            this.inner.push(s);
        }

        this.inner.poll_next_unpin(cx)
    }
}
