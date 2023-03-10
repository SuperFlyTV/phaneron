use std::sync::Arc;

use crate::{
    channel::{Channel, ChannelSemaphore},
    graph::AudioOutputId,
    node_context::{FrameContext, NodeContext},
};

use super::audio_frame::AudioFrame;

pub struct AudioOutput {
    context: NodeContext,
    inner: Arc<AudioOutputInner>,
}

impl AudioOutput {
    pub fn new(context: NodeContext, channel: Channel<AudioFrame>) -> Self {
        Self {
            context,
            inner: Arc::new(AudioOutputInner { channel }),
        }
    }

    pub async fn push_frame(&self, _context: &FrameContext, frame: AudioFrame) {
        self.inner.channel.send(&self.context, frame).await;
    }
}

struct AudioOutputInner {
    channel: Channel<AudioFrame>,
}

pub struct AudioPipe {
    pub id: AudioOutputId,
    pub receiver: tokio::sync::mpsc::Receiver<(AudioFrame, ChannelSemaphore)>,
}

impl AudioPipe {
    pub fn new(
        id: AudioOutputId,
        receiver: tokio::sync::mpsc::Receiver<(AudioFrame, ChannelSemaphore)>,
    ) -> Self {
        Self { id, receiver }
    }

    pub async fn next_frame(&mut self) -> Option<(AudioFrame, ChannelSemaphore)> {
        self.receiver.recv().await
    }
}
