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
use std::thread::JoinHandle;
use std::time::SystemTime;
use std::{net::SocketAddr, sync::Arc, time::Duration};

use abi_stable::sabi_trait::TD_Opaque;
use abi_stable::std_types::{ROption, RString};
use decklink::device::output::{
    DecklinkOutputDevice, DecklinkOutputDeviceVideoScheduled, DecklinkOutputDeviceVideoSync,
    DecklinkVideoOutputFlags,
};
use decklink::device::DecklinkDevice;
use decklink::display_mode::DecklinkDisplayModeId;
use phaneron_plugin::types::{FromAudioF32, FromRGBA, NodeContext};
use phaneron_plugin::{
    traits::Node_TO, types::Node, types::ProcessFrameContext, AudioChannelLayout, AudioFormat,
    AudioInputId, ColourSpace, InterlaceMode, VideoFormat, VideoInputId,
};
use serde::{Deserialize, Serialize};
// use tokio::time::{Instant, MissedTickBehavior};
use tracing::{debug, info};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DecklinkConsumerConfiguration {
    pub device_index: usize,
}

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
    // pub output: DecklinkOutputDevice,
    // pub video: Box<dyn DecklinkOutputDeviceVideoSync>,
}

pub struct DecklinkConsumer {
    node_id: String,
    context: NodeContext,
    configuration: DecklinkConsumerConfiguration,

    video_input: VideoInputId,
    audio_input: AudioInputId,

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
            decklink: Mutex::default(),
        }
    }
}

impl phaneron_plugin::traits::Node for DecklinkConsumer {
    fn apply_state(&self, _state: RString) -> bool {
        let mut current_device = self.decklink.lock().unwrap();
        if current_device.is_some() {
            return false;
        }

        let configuration = self.configuration.clone();

        let decklink_thread = std::thread::spawn(move || {
            let decklink_devices = decklink::device::get_devices().expect("Device query failed");
            let decklink_device = decklink_devices
                .into_iter()
                .nth(configuration.device_index)
                .expect("Invalid device index");

            info!(
                "Opened decklink #{} {:?} {:?}",
                configuration.device_index,
                decklink_device.model_name(),
                decklink_device.display_name()
            );

            let output = decklink_device.output().expect("Failed to open output");

            let video_output = output
                .enable_video_output_sync(
                    DecklinkDisplayModeId::HD1080p50,
                    DecklinkVideoOutputFlags::empty(),
                )
                .expect("Failed to enable video output");

            loop {
                //
            }

            //
        });

        *current_device = Some(DecklinkOutputWrapper {
            thread: decklink_thread,
        });

        false
    }
    fn process_frame(&self, frame_context: ProcessFrameContext) {
        let video_input = frame_context.get_video_input(&self.video_input).unwrap();

        let device = self.decklink.lock().unwrap();
        if let Some(device) = &*device {
            // TODO
            info!("TODO frame");
        }
    }
}
