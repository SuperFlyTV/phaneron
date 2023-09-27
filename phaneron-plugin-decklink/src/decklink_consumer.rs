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

use std::sync::mpsc::SyncSender;
use std::sync::Mutex;
use std::thread::JoinHandle;
use std::time::SystemTime;

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{ROption, RString, RVec};
use decklink::frame::DecklinkAlignedVec;
use phaneron_plugin::types::{FromRGBA, NodeContext};
use phaneron_plugin::{
    traits::Node_TO, types::Node, types::ProcessFrameContext, AudioInputId, ColourSpace,
    InterlaceMode, VideoFormat, VideoInputId,
};
use tracing::{debug, info};

use crate::decklink_consumer_config::DecklinkConsumerConfiguration;
use crate::decklink_consumer_thread::{
    create_decklink_thread, DecklinkThreadMessage, VideoFrameMessage,
};

pub struct DecklinkConsumerHandle {
    node_id: String,
}
impl DecklinkConsumerHandle {
    pub(super) fn new(node_id: String) -> Self {
        Self { node_id }
    }
}
impl phaneron_plugin::traits::NodeHandle for DecklinkConsumerHandle {
    fn initialize(&self, context: NodeContext, configuration: ROption<RString>) -> Node {
        let configuration: DecklinkConsumerConfiguration =
            serde_json::from_str(&configuration.unwrap()).unwrap();

        let node = DecklinkConsumer::new(self.node_id.clone(), context, configuration);

        Node_TO::from_value(node, TD_Opaque)
    }
}

struct DecklinkOutputWrapper {
    pub thread: JoinHandle<()>,
    pub send: SyncSender<DecklinkThreadMessage>,
}

pub struct DecklinkConsumer {
    node_id: String,
    context: NodeContext,
    configuration: DecklinkConsumerConfiguration,

    video_input: VideoInputId,
    audio_input: AudioInputId,

    from_rgba: Mutex<Option<FromRGBA>>,

    decklink: Mutex<Option<DecklinkOutputWrapper>>,
}

impl DecklinkConsumer {
    pub fn new(
        node_id: String,
        context: NodeContext,
        configuration: DecklinkConsumerConfiguration,
    ) -> Self {
        // TODO - can these be done in `apply_state` once we know something about the device?
        let video_input = context.add_video_input();
        let audio_input = context.add_audio_input();

        Self {
            node_id,
            context,
            configuration,

            video_input,
            audio_input,

            from_rgba: Default::default(),

            decklink: Mutex::default(),
        }
    }

    pub fn destroy(&mut self) {
        let mut device = self.decklink.lock().unwrap();
        if let Some(device) = device.take() {
            device
                .send
                .send(DecklinkThreadMessage::Terminate)
                .expect("Send failed");

            device.thread.join().expect("Join failed");
        }
    }
}

impl phaneron_plugin::traits::Node for DecklinkConsumer {
    fn apply_state(&self, _state: RString) -> bool {
        let mut current_device = self.decklink.lock().unwrap();
        if current_device.is_some() {
            return false;
        }

        let (decklink_thread, message_sender) = create_decklink_thread(self.configuration.clone());

        *current_device = Some(DecklinkOutputWrapper {
            thread: decklink_thread,
            send: message_sender,
        });

        false
    }
    fn process_frame(&self, frame_context: ProcessFrameContext) {
        let video_input = frame_context.get_video_input(&self.video_input).unwrap();

        let mut from_rgba_lock = self.from_rgba.lock().unwrap();
        let from_rgba = from_rgba_lock.get_or_insert_with(|| {
            self.context.create_from_rgba(
                &VideoFormat::BGRA8,
                &ColourSpace::sRGB.colour_spec(),
                1920,
                1080,
                InterlaceMode::Progressive,
            )
        });

        let video_frame = frame_context
            .get_video_input(&self.video_input)
            .unwrap_or(frame_context.get_black_frame())
            .clone();
        let video_frame = from_rgba.process_frame(&frame_context, video_frame.frame);

        let copy_context = frame_context.submit().unwrap();

        let video_frame = from_rgba.copy_frame(&copy_context, video_frame);
        let video_frame = into_decklink_avec(video_frame);

        let device: std::sync::MutexGuard<'_, Option<DecklinkOutputWrapper>> =
            self.decklink.lock().unwrap();
        if let Some(device) = &*device {
            device
                .send
                .send(DecklinkThreadMessage::VideoFrame(VideoFrameMessage {
                    frame: video_frame,
                }))
                .expect("Send failed");
        }
    }
}

fn into_decklink_avec(input: RVec<RVec<u8>>) -> DecklinkAlignedVec {
    let total_size: usize = input.iter().map(|v| v.len()).sum();

    // HACK: should not use internal methods like this
    let mut result = DecklinkAlignedVec::__from_elem(64, 0, total_size);

    // This is a crude attempt to perform the concat in a reasonably performant way
    let mut offset = 0;
    for buffer in input {
        result[offset..(offset + buffer.len())].copy_from_slice(&buffer);
        offset += buffer.len()
    }

    result
}
