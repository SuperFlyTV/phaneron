extern crate ffmpeg_the_third as ffmpeg;
use std::collections::HashMap;
use std::sync::Mutex;
use std::thread::JoinHandle;

use anyhow::anyhow;
use async_trait::async_trait;
use byteorder::{ByteOrder, LittleEndian};
use lazy_static::__Deref;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use tracing::info;
use tracing::log::debug;

use crate::compute::audio_frame::{AudioFrame, AudioFrameId};
use crate::compute::audio_stream::AudioOutput;
use crate::compute::video_frame::VideoFrame;
use crate::compute::video_stream::VideoOutput;
use crate::format::VideoFormat;
use crate::graph::{AudioInputId, AudioOutputId, VideoOutputId};
use crate::io::ToRGBA;
use crate::node_context::{Node, ProcessFrameContext};
use crate::yadif::{Yadif, YadifConfig, YadifMode};
use crate::VideoInputId;
use crate::{colour::ColourSpace, graph::NodeId, node_context::NodeContext};

const READ_BUFFER_SIZE: usize = 2;

pub struct FFmpegProducerPlugin {}
impl FFmpegProducerPlugin {
    pub fn load() -> Self {
        Self {}
    }

    pub async fn initialize(&mut self) {
        info!("FFmpeg Producer plugin initializing");
        ffmpeg::init().unwrap();
        info!("FFmpeg Producer plugin initialized");
    }

    pub async fn create_node(&mut self, node_id: NodeId) -> FFmpegProducer {
        FFmpegProducer::new(node_id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FFmpegProducerState {}

pub struct FFmpegProducerConfiguration {
    pub file: String,
}

type FFmpegAudioProcess = (Mutex<std::sync::mpsc::Receiver<AudioFrame>>, AudioOutput);
type FFmpegVideoProcess = (Mutex<std::sync::mpsc::Receiver<VideoFrame>>, VideoOutput);

pub struct FFmpegProducer {
    node_id: NodeId,
    read_thread: Option<JoinHandle<()>>,
    loader_threads: Option<Vec<JoinHandle<()>>>,
    state: Mutex<Option<FFmpegProducerState>>,
    audio_processes: tokio::sync::Mutex<Option<Vec<FFmpegAudioProcess>>>,
    video_processes: tokio::sync::Mutex<Option<Vec<FFmpegVideoProcess>>>,
}

impl FFmpegProducer {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            read_thread: None,
            loader_threads: None,
            state: Default::default(),
            audio_processes: Default::default(),
            video_processes: Default::default(),
        }
    }

    pub async fn initialize(
        &mut self,
        context: NodeContext,
        configuration: FFmpegProducerConfiguration,
    ) {
        info!("FFmpeg Producer {} initializing", self.node_id);
        // state_sender.send(initial_state.clone()).unwrap();

        // let initial_state: FFmpegProducerState = serde_json::from_str(&initial_state).unwrap();

        let mut loaded_video_frame_receivers: Vec<std::sync::mpsc::Receiver<VideoFrame>> = vec![];
        let mut loaded_audio_frame_receivers: Vec<std::sync::mpsc::Receiver<AudioFrame>> = vec![];

        // Uses a hashmap so that `stream.index()` can be used in the reading thread
        let mut read_frame_senders: HashMap<
            usize,
            std::sync::mpsc::SyncSender<ffmpeg::packet::Packet>,
        > = HashMap::new();

        let mut load_threads: Vec<JoinHandle<()>> = vec![];

        // let mut ictx = ffmpeg::format::input(&initial_state.media_file).unwrap();
        let mut ictx = ffmpeg::format::input(&configuration.file).unwrap();
        // *self.state.lock().unwrap() = Some(initial_state);

        // TODO: Remove this later, makes the demo work as intended 100% of the time instead of some of the time
        let mut audio_stream: Option<usize> = None;

        // TODO: Flatten this out and make it more readable
        for stream in ictx.streams() {
            let stream_type = stream.codec().medium();
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
                    let context = context.clone();
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
                                        colour_space,
                                        decoded.width() as usize,
                                        decoded.height() as usize,
                                    ) // TODO: Make sure the format, colourspace, width + height haven't changed on us
                                });

                                let inputs = match video_format {
                                    VideoFormat::BRGA8 | VideoFormat::RGBA8 | VideoFormat::V210 => {
                                        vec![decoded.data(0)]
                                    }
                                    VideoFormat::YUV420p
                                    | VideoFormat::YUV422p8
                                    | VideoFormat::YUV422p10 => {
                                        vec![decoded.data(0), decoded.data(1), decoded.data(2)]
                                    }
                                };

                                let interlaced = decoded.is_interlaced();
                                let loaded_frame = to_rgba.load_frame(&inputs);
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
                    let thread = std::thread::spawn(move || loop {
                        let packet = read_frame_receiver.recv().unwrap();
                        audio_decoder.send_packet(&packet).unwrap();

                        let mut decoded = ffmpeg::frame::Audio::empty();
                        let frame = audio_decoder.receive_frame(&mut decoded);

                        if frame.is_ok() {
                            let planes = decoded.planes();
                            let mut audio_buffers: Vec<Vec<f32>> = Vec::with_capacity(planes);
                            for i in 0..planes {
                                let decoded_data = decoded.data(i);
                                // TODO: Use decoded.format() to check what kind of conversion to do
                                // TODO: Similar conversion methods as video frames have to go through
                                let mut audio_buffer = vec![0i32; decoded_data.len() / 4];
                                LittleEndian::read_i32_into(decoded_data, &mut audio_buffer);
                                let processed_buffer: Vec<f32> = audio_buffer
                                    .iter()
                                    .map(|sample| ((*sample as f64) / i32::MAX as f64) as f32)
                                    .collect();

                                audio_buffers.push(processed_buffer)
                            }
                            let audio_frame =
                                AudioFrame::new(AudioFrameId::default(), audio_buffers);
                            loaded_frame_sender.send(audio_frame).unwrap();
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
            let video_stream = context.add_video_output().await;
            video_processes.push((Mutex::new(receiver), video_stream));
        }

        for receiver in loaded_audio_frame_receivers {
            let audio_stream = context.add_audio_output().await;
            audio_processes.push((Mutex::new(receiver), audio_stream));
        }

        debug!(
            "FFmpeg producer {} loaded file with {} video streams and {} audio streams",
            self.node_id,
            video_processes.len(),
            audio_processes.len()
        );

        self.read_thread = Some(reader_thread);
        self.loader_threads = Some(load_threads);
        self.video_processes.lock().await.replace(video_processes);
        self.audio_processes.lock().await.replace(audio_processes);

        info!("FFmpeg Producer {} initialized", self.node_id);
    }
}

#[async_trait]
impl Node for FFmpegProducer {
    async fn apply_state(&self, state: String) -> bool {
        false
    }
    async fn process_frame(
        &self,
        context: ProcessFrameContext,
        video_frames: HashMap<VideoInputId, (VideoOutputId, VideoFrame)>,
        audio_frames: HashMap<AudioInputId, (AudioOutputId, AudioFrame)>,
        black_frame: (VideoOutputId, VideoFrame),
        silence_frame: (AudioOutputId, AudioFrame),
    ) {
        let frame_context = context.submit().await;

        let video_processes_lock = self.video_processes.lock().await;
        if let Some(video_processes) = &*video_processes_lock {
            for (video_receiver, video_stream) in video_processes.iter() {
                let frame = video_receiver.lock().unwrap().recv().unwrap();
                video_stream.push_frame(&frame_context, frame).await;
            }
        }

        let audio_processes_lock = self.audio_processes.lock().await;
        if let Some(audio_processes) = &*audio_processes_lock {
            for (audio_receiver, audio_stream) in audio_processes.iter() {
                let frame = audio_receiver.lock().unwrap().recv().unwrap();
                audio_stream.push_frame(&frame_context, frame).await;
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
            ffmpeg::format::Pixel::BGRA => Ok(VideoFormat::BRGA8),
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
