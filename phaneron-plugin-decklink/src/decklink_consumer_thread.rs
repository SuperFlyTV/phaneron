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
use std::thread::JoinHandle;

use decklink::device::output::DecklinkVideoOutputFlags;
use decklink::display_mode::DecklinkDisplayModeId;

use decklink::frame::{
    DecklinkAlignedVec, DecklinkFrameFlags, DecklinkPixelFormat, DecklinkVideoMutableFrame,
};
// use tokio::time::{Instant, MissedTickBehavior};
use tracing::info;

use crate::decklink_consumer_config::DecklinkConsumerConfiguration;

const MESSAGE_BUFFER_SIZE: usize = 2;
pub enum DecklinkThreadMessage {
    Terminate,
    VideoFrame(VideoFrameMessage),
}

pub struct VideoFrameMessage {
    pub frame: DecklinkAlignedVec,
}

pub fn create_decklink_thread(
    configuration: DecklinkConsumerConfiguration,
) -> (JoinHandle<()>, SyncSender<DecklinkThreadMessage>) {
    let (message_sender, message_receiver) =
        std::sync::mpsc::sync_channel::<DecklinkThreadMessage>(MESSAGE_BUFFER_SIZE);

    let thread = std::thread::spawn(move || {
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
            let packet = message_receiver.recv().unwrap();

            match packet {
                DecklinkThreadMessage::Terminate => break,
                DecklinkThreadMessage::VideoFrame(frame) => {
                    // info!("frame {} bytes!", frame.frame.len());

                    let mut decklink_frame = Box::new(DecklinkVideoMutableFrame::create(
                        1920,
                        1080,
                        1920 * 4,
                        DecklinkPixelFormat::Format8BitBGRA,
                        DecklinkFrameFlags::empty(),
                    ));
                    decklink_frame.set_bytes(frame.frame).unwrap();

                    video_output.display_custom_frame(decklink_frame).unwrap();
                }
            }
        }

        // TODO - cleanup?
    });

    (thread, message_sender)
}
