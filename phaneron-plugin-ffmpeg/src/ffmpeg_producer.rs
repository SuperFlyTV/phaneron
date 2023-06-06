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

extern crate ffmpeg_the_third as ffmpeg;
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread::JoinHandle;

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{ROption, RSlice, RString},
};
use anyhow::anyhow;
use phaneron_plugin::{
    traits::Node_TO, types::AudioFrame, types::AudioOutput, types::Node, types::NodeContext,
    types::ProcessFrameContext, types::ToAudioF32, types::ToRGBA, types::VideoFrame,
    types::VideoOutput, AudioChannelLayout, AudioFormat, ColourSpace, VideoFormat,
};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use tracing::{debug, info};

use phaneron_plugin_utils::yadif::{Yadif, YadifConfig, YadifMode};

const READ_BUFFER_SIZE: usize = 2;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FFmpegProducerState {
    pub file: String,
}

type FFmpegAudioProcess = (Mutex<std::sync::mpsc::Receiver<AudioFrame>>, AudioOutput);
type FFmpegVideoProcess = (Mutex<std::sync::mpsc::Receiver<VideoFrame>>, VideoOutput);

pub struct FFmpegProducerHandle {
    node_id: String,
}
impl FFmpegProducerHandle {
    pub(super) fn new(node_id: String) -> Self {
        Self { node_id }
    }
}
impl phaneron_plugin::traits::NodeHandle for FFmpegProducerHandle {
    fn initialize(&self, context: NodeContext, _configuration: ROption<RString>) -> Node {
        let node = FFmpegProducer::new(self.node_id.clone(), context);

        Node_TO::from_value(node, TD_Opaque)
    }
}

pub struct FFmpegProducer {
    node_id: String,
    context: NodeContext,
    read_thread: Mutex<Option<JoinHandle<()>>>,
    loader_threads: Mutex<Option<Vec<JoinHandle<()>>>>,
    state: Mutex<Option<FFmpegProducerState>>,
    audio_processes: Mutex<Option<Vec<FFmpegAudioProcess>>>,
    video_processes: Mutex<Option<Vec<FFmpegVideoProcess>>>,
}

impl FFmpegProducer {
    pub fn new(node_id: String, context: NodeContext) -> Self {
        Self {
            node_id,
            context,
            read_thread: Default::default(),
            loader_threads: Default::default(),
            state: Default::default(),
            audio_processes: Default::default(),
            video_processes: Default::default(),
        }
    }
}

impl phaneron_plugin::traits::Node for FFmpegProducer {
    fn apply_state(&self, state: RString) -> bool {
        let current_state = self.state.lock().unwrap();
        if current_state.is_some() {
            return false;
        }

        let state: FFmpegProducerState = serde_json::from_str(&state).unwrap();

        let mut loaded_video_frame_receivers: Vec<std::sync::mpsc::Receiver<VideoFrame>> = vec![];
        let mut loaded_audio_frame_receivers: Vec<std::sync::mpsc::Receiver<AudioFrame>> = vec![];

        // Uses a hashmap so that `stream.index()` can be used in the reading thread
        let mut read_frame_senders: HashMap<
            usize,
            std::sync::mpsc::SyncSender<ffmpeg::packet::Packet>,
        > = HashMap::new();

        let mut load_threads: Vec<JoinHandle<()>> = vec![];

        // let mut ictx = ffmpeg::format::input(&initial_state.media_file).unwrap();
        let mut ictx = ffmpeg::format::input(&state.file).unwrap();
        // *self.state.lock().unwrap() = Some(initial_state);

        // TODO: Remove this later, makes the demo work as intended 100% of the time instead of some of the time
        let mut audio_stream: Option<usize> = None;

        // TODO: Flatten this out and make it more readable
        for stream in ictx.streams() {
            let stream_type = stream.parameters().medium();
            match stream_type {
                ffmpeg::media::Type::Video => {
                    let video_decoder_context =
                        ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                            .unwrap();
                    let mut video_decoder = video_decoder_context.decoder().video().unwrap();
                    let (loaded_frame_sender, loaded_frame_receiver) =
                        std::sync::mpsc::sync_channel(1);
                    loaded_video_frame_receivers.push(loaded_frame_receiver);
                    let (read_frame_sender, read_frame_receiver) =
                        std::sync::mpsc::sync_channel::<ffmpeg::packet::Packet>(READ_BUFFER_SIZE);
                    read_frame_senders.insert(stream.index(), read_frame_sender);
                    let context = self.context.clone();
                    let thread = std::thread::spawn(move || {
                        let mut to_rgba: Option<ToRGBA> = None;
                        let mut yadif: Option<Yadif> = None;
                        let mut colour_space: Option<ColourSpace> = None;
                        let mut video_format: Option<VideoFormat> = None;
                        loop {
                            let packet = read_frame_receiver.recv().unwrap();
                            video_decoder.send_packet(&packet).unwrap();

                            let mut decoded = ffmpeg::frame::Video::empty();
                            let frame = video_decoder.receive_frame(&mut decoded);

                            if frame.is_ok() {
                                let colour_space = colour_space.get_or_insert_with(|| {
                                    FFmegColourSpace(decoded.color_space()).try_into().unwrap()
                                });
                                let video_format = video_format.get_or_insert_with(|| {
                                    FFmpegPixelFormat(decoded.format()).try_into().unwrap()
                                });
                                let to_rgba = to_rgba.get_or_insert_with(|| {
                                    context.create_to_rgba(
                                        video_format,
                                        &colour_space.colour_spec(),
                                        decoded.width() as usize,
                                        decoded.height() as usize,
                                    ) // TODO: Make sure the format, colourspace, width + height haven't changed on us
                                });

                                let inputs: Vec<RSlice<u8>> = match video_format {
                                    VideoFormat::BGRA8 | VideoFormat::RGBA8 | VideoFormat::V210 => {
                                        vec![decoded.data(0).into()]
                                    }
                                    VideoFormat::YUV420p
                                    | VideoFormat::YUV422p8
                                    | VideoFormat::YUV422p10 => {
                                        vec![
                                            decoded.data(0).into(),
                                            decoded.data(1).into(),
                                            decoded.data(2).into(),
                                        ]
                                    }
                                };

                                let interlaced = decoded.is_interlaced();
                                let loaded_frame = to_rgba.load_frame(&inputs.as_slice().into());
                                let frame = to_rgba.process_frame(loaded_frame);

                                if interlaced {
                                    let yadif = yadif.get_or_insert_with(|| {
                                        let yadif_mode = YadifMode::Field;
                                        let tff = decoded.is_top_first();
                                        Yadif::new(
                                            &context,
                                            decoded.width() as usize,
                                            decoded.height() as usize,
                                            YadifConfig {
                                                mode: yadif_mode,
                                                tff,
                                            },
                                        )
                                    });
                                    if let Some(frame) = yadif.run(&frame).first() {
                                        loaded_frame_sender.send(frame.clone()).unwrap();
                                    }
                                } else {
                                    loaded_frame_sender.send(frame).unwrap();
                                }
                            }
                        }
                    });
                    load_threads.push(thread);
                }
                ffmpeg::media::Type::Audio => {
                    let wanted_audio_index = audio_stream.get_or_insert_with(|| {
                        info!(
                            "FFmpeg producer {} using audio stream {}",
                            self.node_id,
                            stream.index()
                        );
                        stream.index()
                    });
                    if stream.index() != *wanted_audio_index {
                        continue;
                    }
                    let audio_decoder_context =
                        ffmpeg::codec::context::Context::from_parameters(stream.parameters())
                            .unwrap();
                    let mut audio_decoder = audio_decoder_context.decoder().audio().unwrap();
                    let (loaded_frame_sender, loaded_frame_receiver) =
                        std::sync::mpsc::sync_channel(1);
                    loaded_audio_frame_receivers.push(loaded_frame_receiver);
                    let (read_frame_sender, read_frame_receiver) =
                        std::sync::mpsc::sync_channel::<ffmpeg::packet::Packet>(READ_BUFFER_SIZE);
                    read_frame_senders.insert(stream.index(), read_frame_sender);
                    let context = self.context.clone();
                    let thread = std::thread::spawn(move || {
                        let mut to_audio_f32: Option<ToAudioF32> = None;
                        loop {
                            let packet = read_frame_receiver.recv().unwrap();
                            audio_decoder.send_packet(&packet).unwrap();

                            let mut decoded = ffmpeg::frame::Audio::empty();
                            let frame = audio_decoder.receive_frame(&mut decoded);

                            if frame.is_ok() {
                                let planes = decoded.planes();
                                for i in 0..planes {
                                    let decoded_data = decoded.data(i);
                                    let to_audio_f32 = to_audio_f32.get_or_insert_with(|| {
                                        context.create_to_audio_f32(
                                            AudioFormat::I32,
                                            AudioChannelLayout::Mono,
                                        )
                                    });
                                    let loaded_frame =
                                        to_audio_f32.load_frame(&decoded_data.into());
                                    let audio_frame = to_audio_f32.process_frame(loaded_frame);
                                    loaded_frame_sender.send(audio_frame).unwrap();
                                }
                            }
                        }
                    });
                    load_threads.push(thread);
                }
                _ => {}
            }
        }

        let reader_thread = std::thread::spawn(move || loop {
            let packets = ictx.packets();
            for (stream, packet) in packets {
                if let Some(sender) = read_frame_senders.get(&stream.index()) {
                    sender.send(packet).unwrap();
                }
            }
            ictx.seek(0, std::ops::RangeFull).unwrap();
        });

        let mut video_processes: Vec<(Mutex<std::sync::mpsc::Receiver<VideoFrame>>, VideoOutput)> =
            vec![];
        let mut audio_processes: Vec<(Mutex<std::sync::mpsc::Receiver<AudioFrame>>, AudioOutput)> =
            vec![];

        for receiver in loaded_video_frame_receivers {
            let video_output = self.context.add_video_output();
            video_processes.push((Mutex::new(receiver), video_output));
        }

        for receiver in loaded_audio_frame_receivers {
            let audio_output = self.context.add_audio_output();
            audio_processes.push((Mutex::new(receiver), audio_output));
        }

        debug!(
            "FFmpeg producer {} loaded file with {} video streams and {} audio streams",
            self.node_id,
            video_processes.len(),
            audio_processes.len()
        );

        self.read_thread.lock().unwrap().replace(reader_thread);
        self.loader_threads.lock().unwrap().replace(load_threads);
        self.video_processes
            .lock()
            .unwrap()
            .replace(video_processes);
        self.audio_processes
            .lock()
            .unwrap()
            .replace(audio_processes);

        true
    }

    fn process_frame(&self, context: ProcessFrameContext) {
        let frame_context = context.submit().unwrap();

        let video_processes_lock = self.video_processes.lock().unwrap();
        if let Some(video_processes) = &*video_processes_lock {
            for (video_receiver, video_output) in video_processes.iter() {
                let frame = video_receiver.lock().unwrap().recv().unwrap();
                video_output.push_frame(&frame_context, frame);
            }
        }

        let audio_processes_lock = self.audio_processes.lock().unwrap();
        if let Some(audio_processes) = &*audio_processes_lock {
            for (audio_receiver, audio_output) in audio_processes.iter() {
                let frame = audio_receiver.lock().unwrap().recv().unwrap();
                audio_output.push_frame(&frame_context, frame);
            }
        }
    }
}

struct FFmegColourSpace(ffmpeg::color::Space);

impl Deref for FFmegColourSpace {
    type Target = ffmpeg::color::Space;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<FFmegColourSpace> for ColourSpace {
    type Error = anyhow::Error;

    fn try_from(value: FFmegColourSpace) -> Result<Self, Self::Error> {
        match *value {
            ffmpeg::color::Space::RGB => Ok(ColourSpace::sRGB),
            ffmpeg::color::Space::BT709 => Ok(ColourSpace::BT_709),
            ffmpeg::color::Space::Unspecified => Ok(ColourSpace::BT_709),
            _ => Err(anyhow!(
                "Unsupported colour space: {}",
                value.deref().name().unwrap_or("Unknown")
            )),
        }
    }
}

struct FFmpegPixelFormat(ffmpeg::format::Pixel);

impl Deref for FFmpegPixelFormat {
    type Target = ffmpeg::format::Pixel;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl TryFrom<FFmpegPixelFormat> for VideoFormat {
    type Error = anyhow::Error;

    fn try_from(value: FFmpegPixelFormat) -> Result<Self, Self::Error> {
        match *value {
            ffmpeg::format::Pixel::BGRA => Ok(VideoFormat::BGRA8),
            ffmpeg::format::Pixel::RGBA => Ok(VideoFormat::RGBA8),
            ffmpeg::format::Pixel::YUV420P => Ok(VideoFormat::YUV420p),
            ffmpeg::format::Pixel::YUV422P => Ok(VideoFormat::YUV422p8),
            ffmpeg::format::Pixel::YUV422P10 | ffmpeg::format::Pixel::YUV422P10LE => {
                Ok(VideoFormat::YUV422p10)
            }
            _ => Err(anyhow!(
                "Unsupported pixel format: {}",
                value.deref().descriptor().unwrap().name()
            )),
        }
    }
}
