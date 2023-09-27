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

use std::net::Ipv4Addr;
use std::sync::Mutex;
use std::time::SystemTime;
use std::{net::SocketAddr, sync::Arc, time::Duration};

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{ROption, RString};
use phaneron_plugin::types::{FromAudioF32, FromRGBA, NodeContext};
use phaneron_plugin::{
    traits::Node_TO, types::Node, types::ProcessFrameContext, AudioChannelLayout, AudioFormat,
    AudioInputId, ColourSpace, InterlaceMode, VideoFormat, VideoInputId,
};
// use tokio::time::{Instant, MissedTickBehavior};
use tracing::{debug, info};

pub struct DecklinkConsumerHandle {
    node_id: String,
}
impl DecklinkConsumerHandle {
    pub(super) fn new(node_id: String) -> Self {
        Self { node_id }
    }
}
impl phaneron_plugin::traits::NodeHandle for DecklinkConsumerHandle {
    fn initialize(&self, context: NodeContext, _configuration: ROption<RString>) -> Node {
        let node = DecklinkConsumer::new(self.node_id.clone(), context);

        Node_TO::from_value(node, TD_Opaque)
    }
}

pub struct DecklinkConsumer {
    node_id: String,
    context: NodeContext,

    video_input: VideoInputId,
    audio_input: AudioInputId,
}

impl DecklinkConsumer {
    pub fn new(node_id: String, context: NodeContext) -> Self {
        // TODO - can these be done in `apply_state` once we know something about the device?
        let video_input = context.add_video_input();
        let audio_input = context.add_audio_input();

        Self {
            node_id,
            context,
            video_input,
            audio_input,
        }
    }
}

impl phaneron_plugin::traits::Node for DecklinkConsumer {
    fn apply_state(&self, state: RString) -> bool {
        false
    }
    fn process_frame(&self, frame_context: ProcessFrameContext) {
        // TODO
    }
}
