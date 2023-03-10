use std::sync::Arc;

use crate::{
    channel::{Channel, ChannelSemaphore},
    graph::VideoOutputId,
    node_context::{FrameContext, NodeContext},
};

use super::video_frame::VideoFrame;

pub struct VideoOutput {
    context: NodeContext,
    inner: Arc<VideoOutputInner>,
}

impl VideoOutput {
    pub fn new(context: NodeContext, channel: Channel<VideoFrame>) -> Self {
        Self {
            context,
            inner: Arc::new(VideoOutputInner { channel }),
        }
    }

    pub async fn push_frame(&self, _context: &FrameContext, frame: VideoFrame) {
        self.inner.channel.send(&self.context, frame).await;
    }
}

struct VideoOutputInner {
    channel: Channel<VideoFrame>,
}

pub struct VideoPipe {
    pub id: VideoOutputId,
    pub receiver: tokio::sync::mpsc::Receiver<(VideoFrame, ChannelSemaphore)>,
}

impl VideoPipe {
    pub fn new(
        id: VideoOutputId,
        receiver: tokio::sync::mpsc::Receiver<(VideoFrame, ChannelSemaphore)>,
    ) -> Self {
        Self { id, receiver }
    }

    pub async fn next_frame(&mut self) -> Option<(VideoFrame, ChannelSemaphore)> {
        self.receiver.recv().await
    }
}
