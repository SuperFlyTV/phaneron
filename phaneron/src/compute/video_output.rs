/*
 * Phaneron media compositing software.
 * Copyright (C) 2023 SuperFlyTV AB
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use std::sync::Arc;

use phaneron_plugin::VideoOutputId;

use crate::channel::{Channel, ChannelSemaphore, ChannelSemaphoreProvider};

#[derive(Debug, Clone)]
pub struct VideoOutput {
    semaphore_provider: ChannelSemaphoreProvider,
    inner: Arc<VideoOutputInner>,
}

impl VideoOutput {
    pub fn new(
        semaphore_provider: ChannelSemaphoreProvider,
        channel: Channel<phaneron_plugin::types::VideoFrame>,
    ) -> Self {
        Self {
            semaphore_provider,
            inner: Arc::new(VideoOutputInner { channel }),
        }
    }
}

impl phaneron_plugin::traits::VideoOutput for VideoOutput {
    fn push_frame(
        &self,
        context: &phaneron_plugin::types::FrameContext,
        frame: phaneron_plugin::types::VideoFrame,
    ) {
        self.inner.channel.send(&self.semaphore_provider, frame);
    }
}

#[derive(Debug)]
struct VideoOutputInner {
    channel: Channel<phaneron_plugin::types::VideoFrame>,
}

pub struct VideoPipe {
    pub id: VideoOutputId,
    pub receiver:
        tokio::sync::mpsc::Receiver<(phaneron_plugin::types::VideoFrame, ChannelSemaphore)>,
}

impl VideoPipe {
    pub fn new(
        id: VideoOutputId,
        receiver: tokio::sync::mpsc::Receiver<(
            phaneron_plugin::types::VideoFrame,
            ChannelSemaphore,
        )>,
    ) -> Self {
        Self { id, receiver }
    }

    pub async fn next_frame(
        &mut self,
    ) -> Option<(phaneron_plugin::types::VideoFrame, ChannelSemaphore)> {
        self.receiver.recv().await
    }
}
