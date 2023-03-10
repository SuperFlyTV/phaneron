use std::sync::Arc;
use tokio::sync::Mutex;

use crate::node_context::NodeContext;

pub struct Channel<T>
where
    T: Clone,
{
    inner: Arc<Mutex<ChannelInner<T>>>,
}

impl<T> Channel<T>
where
    T: Clone,
{
    pub async fn subscribe(&self) -> tokio::sync::mpsc::Receiver<(T, ChannelSemaphore)> {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let mut inner = self.inner.lock().await;
        inner.senders.push(sender);
        receiver
    }

    pub async fn send(&self, context: &NodeContext, value: T) {
        let inner = self.inner.lock().await;
        for sender in &inner.senders {
            let semaphore = context.get_frame_semaphore().await;
            sender.send((value.clone(), semaphore)).await.ok();
        }
    }

    pub async fn no_receivers(&self) -> bool {
        self.inner.lock().await.senders.is_empty()
    }
}

impl<T> Clone for Channel<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Default for Channel<T>
where
    T: Clone,
{
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ChannelInner::new())),
        }
    }
}

// TODO: Is this still needed?
unsafe impl<T> Send for Channel<T> where T: Clone {}
unsafe impl<T> Sync for Channel<T> where T: Clone {}

struct ChannelInner<T>
where
    T: Clone,
{
    senders: Vec<tokio::sync::mpsc::Sender<(T, ChannelSemaphore)>>,
}

impl<T> ChannelInner<T>
where
    T: Clone,
{
    fn new() -> Self {
        ChannelInner { senders: vec![] }
    }
}

// TODO: Is this still needed?
unsafe impl<T> Send for ChannelInner<T> where T: Clone {}
unsafe impl<T> Sync for ChannelInner<T> where T: Clone {}

pub struct ChannelSemaphore {
    inner: tokio::sync::oneshot::Sender<()>,
}

impl ChannelSemaphore {
    pub fn new(semaphore: tokio::sync::oneshot::Sender<()>) -> Self {
        Self { inner: semaphore }
    }

    pub async fn signal(self) {
        self.inner.send(()).ok();
    }
}
