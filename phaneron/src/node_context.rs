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

use std::{collections::HashMap, sync::Arc};

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{
        RArc,
        RResult::{self, RErr, ROk},
        RStr, RString,
    },
};
use phaneron_plugin::{
    AudioChannelLayout, AudioFrameWithId, AudioInputId, AudioOutputId, ColourSpec, InterlaceMode,
    VideoFrameWithId, VideoInputId, VideoOutputId,
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender},
    Mutex,
};

use crate::{
    channel::{Channel, ChannelSemaphore, ChannelSemaphoreProvider},
    compute::{
        audio_frame::{AudioFrame, AudioFrameId},
        audio_output::{AudioOutput, AudioPipe},
        video_output::{VideoOutput, VideoPipe},
        PhaneronComputeContext,
    },
    format::VideoFormat,
    graph::NodeId,
    io::{FromAudioF32, FromRGBA, ToAudioF32, ToRGBA},
};

#[derive(Clone)]
pub struct NodeRunContext {
    node_id: NodeId,
    inner: NodeRunContextInner,
}

impl NodeRunContext {
    pub fn new(
        node_id: NodeId,
        state_tx: tokio::sync::mpsc::UnboundedSender<NodeStateEvent>,
    ) -> Self {
        Self {
            node_id,
            inner: NodeRunContextInner {
                audio_input_ids: Default::default(),
                audio_outputs: Default::default(),
                video_input_ids: Default::default(),
                video_outputs: Default::default(),
                connected_audio_pipes: Default::default(),
                connected_video_pipes: Default::default(),
                state_tx,
                pending_state: Default::default(),
            },
        }
    }

    pub async fn get_run_process_frame_context(&self) -> RunProcessFrameContext {
        let connected_audio_pipes = self.inner.connected_audio_pipes.clone();
        let connected_video_pipes = self.inner.connected_video_pipes.clone();
        let audio_input_ids = self.inner.audio_input_ids.clone();
        let video_input_ids = self.inner.video_input_ids.clone();
        let video_outputs = self.inner.video_outputs.clone();
        let audio_outputs = self.inner.audio_outputs.clone();

        let audio_input_ids = audio_input_ids.lock().await.clone();
        let audio_outputs = audio_outputs.lock().await.clone();
        let video_input_ids = video_input_ids.lock().await.clone();
        let video_outputs = video_outputs.lock().await.clone();

        RunProcessFrameContext::new(
            audio_input_ids,
            audio_outputs,
            video_input_ids,
            video_outputs,
            connected_audio_pipes,
            connected_video_pipes,
        )
    }

    pub async fn set_state(&self, state: String) {
        self.inner.pending_state.lock().await.replace(state);
    }

    pub fn get_pending_state_channel(&self) -> Arc<tokio::sync::Mutex<Option<String>>> {
        self.inner.pending_state.clone()
    }

    pub async fn add_audio_input(&self, input_id: AudioInputId) {
        let mut audio_input_ids = self.inner.audio_input_ids.lock().await;
        audio_input_ids.push(input_id.clone());
        self.inner
            .state_tx
            .send(NodeStateEvent::AudioInputAdded(
                self.node_id.clone(),
                input_id,
            ))
            .ok(); // If receiver is dropped, not much we can do
    }

    pub async fn add_video_input(&self, input_id: VideoInputId) {
        let mut video_input_ids = self.inner.video_input_ids.lock().await;
        video_input_ids.push(input_id.clone());
        self.inner
            .state_tx
            .send(NodeStateEvent::VideoInputAdded(
                self.node_id.clone(),
                input_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do
    }

    pub async fn add_audio_output(
        &self,
        output_id: AudioOutputId,
        channel: Channel<phaneron_plugin::types::AudioFrame>,
    ) {
        self.inner
            .audio_outputs
            .lock()
            .await
            .insert(output_id.clone(), channel.clone());
        self.inner
            .state_tx
            .send(NodeStateEvent::AudioOutputAdded(
                self.node_id.clone(),
                output_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do
    }

    pub async fn add_video_output(
        &self,
        output_id: VideoOutputId,
        channel: Channel<phaneron_plugin::types::VideoFrame>,
    ) {
        self.inner
            .video_outputs
            .lock()
            .await
            .insert(output_id.clone(), channel.clone());
        self.inner
            .state_tx
            .send(NodeStateEvent::VideoOutputAdded(
                self.node_id.clone(),
                output_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do
    }

    pub async fn connect_video_pipe(
        &self,
        to_video_input: &VideoInputId,
        video_pipe: VideoPipe,
    ) -> Result<(), VideoConnectionError> {
        let video_input_ids = self.inner.video_input_ids.lock().await;
        if !video_input_ids.contains(to_video_input) {
            return Err(VideoConnectionError::InputDoesNotExist(
                to_video_input.clone(),
            ));
        }

        let mut connected_video_pipes = self.inner.connected_video_pipes.lock().await;
        if let Some(conn) = connected_video_pipes.get(to_video_input) {
            return Err(VideoConnectionError::InputAlreadyConnectedTo(
                to_video_input.clone(),
                conn.0.clone(),
            ));
        }

        connected_video_pipes.insert(to_video_input.clone(), (video_pipe.id.clone(), video_pipe));

        Ok(())
    }

    pub async fn connect_audio_pipe(
        &self,
        to_audio_input: &AudioInputId,
        audio_pipe: AudioPipe,
    ) -> Result<(), AudioConnectionError> {
        let audio_input_ids = self.inner.audio_input_ids.lock().await;
        if !audio_input_ids.contains(to_audio_input) {
            return Err(AudioConnectionError::InputDoesNotExist(
                to_audio_input.clone(),
            ));
        }

        let mut connected_audio_pipes = self.inner.connected_audio_pipes.lock().await;
        if let Some(conn) = connected_audio_pipes.get(to_audio_input) {
            return Err(AudioConnectionError::InputAlreadyConnectedTo(
                to_audio_input.clone(),
                conn.0.clone(),
            ));
        }

        connected_audio_pipes.insert(to_audio_input.clone(), (audio_pipe.id.clone(), audio_pipe));

        Ok(())
    }

    #[deprecated]
    pub async fn get_available_audio_inputs(&self) -> Vec<AudioInputId> {
        self.inner.audio_input_ids.lock().await.clone()
    }

    #[deprecated]
    pub async fn get_available_video_inputs(&self) -> Vec<VideoInputId> {
        self.inner.video_input_ids.lock().await.clone()
    }

    #[deprecated]
    pub async fn get_available_audio_outputs(&self) -> Vec<AudioOutputId> {
        let audio_outputs = self.inner.audio_outputs.lock().await;

        audio_outputs.keys().cloned().collect()
    }

    #[deprecated]
    pub async fn get_available_video_outputs(&self) -> Vec<VideoOutputId> {
        let video_outputs = self.inner.video_outputs.lock().await;

        video_outputs.keys().cloned().collect()
    }

    pub async fn get_audio_pipe(&self, audio_output_id: &AudioOutputId) -> AudioPipe {
        let audio_outputs = self.inner.audio_outputs.lock().await;
        let audio_output = audio_outputs.get(audio_output_id).unwrap();

        AudioPipe::new(audio_output_id.clone(), audio_output.subscribe().await)
    }

    pub async fn get_video_pipe(&self, video_output_id: &VideoOutputId) -> VideoPipe {
        let video_outputs = self.inner.video_outputs.lock().await;
        let video_output = video_outputs.get(video_output_id).unwrap();

        VideoPipe::new(video_output_id.clone(), video_output.subscribe().await)
    }
}

#[derive(Clone)]
struct NodeRunContextInner {
    audio_input_ids: Arc<Mutex<Vec<AudioInputId>>>,
    audio_outputs: Arc<Mutex<HashMap<AudioOutputId, Channel<phaneron_plugin::types::AudioFrame>>>>,
    video_input_ids: Arc<Mutex<Vec<VideoInputId>>>,
    video_outputs: Arc<Mutex<HashMap<VideoOutputId, Channel<phaneron_plugin::types::VideoFrame>>>>,
    connected_audio_pipes: Arc<Mutex<HashMap<AudioInputId, (AudioOutputId, AudioPipe)>>>,
    connected_video_pipes: Arc<Mutex<HashMap<VideoInputId, (VideoOutputId, VideoPipe)>>>,
    state_tx: tokio::sync::mpsc::UnboundedSender<NodeStateEvent>,
    pending_state: Arc<tokio::sync::Mutex<Option<String>>>,
}

pub struct NodeContextImpl {
    pub node_id: NodeId,
    inner: Arc<NodeContextInner>,
}

impl NodeContextImpl {
    pub fn new(
        node_id: NodeId,
        compute_context: PhaneronComputeContext,
        event_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
        channel_semaphore_provider: ChannelSemaphoreProvider,
    ) -> Self {
        Self {
            node_id: node_id.clone(),
            inner: Arc::new(NodeContextInner {
                node_id,
                compute_context,
                event_tx,
                channel_semaphore_provider,
            }),
        }
    }
}

impl phaneron_plugin::traits::NodeContext for NodeContextImpl {
    fn add_audio_input(&self) -> AudioInputId {
        let audio_input_id = AudioInputId::default();
        self.inner
            .event_tx
            .send(NodeEvent::AudioInputAdded(
                self.node_id.clone(),
                audio_input_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        audio_input_id
    }

    fn add_video_input(&self) -> VideoInputId {
        let video_input_id = VideoInputId::default();
        self.inner
            .event_tx
            .send(NodeEvent::VideoInputAdded(
                self.node_id.clone(),
                video_input_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        video_input_id
    }

    fn add_audio_output(&self) -> phaneron_plugin::types::AudioOutput {
        let audio_output_id = AudioOutputId::default();
        let channel = Channel::default();
        self.inner
            .event_tx
            .send(NodeEvent::AudioOutputAdded(
                self.node_id.clone(),
                audio_output_id,
                channel.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        phaneron_plugin::traits::AudioOutput_TO::from_value(
            AudioOutput::new(self.inner.channel_semaphore_provider.clone(), channel),
            TD_Opaque,
        )
    }

    fn add_video_output(&self) -> phaneron_plugin::types::VideoOutput {
        let video_output_id = VideoOutputId::default();
        let channel = Channel::default();
        self.inner
            .event_tx
            .send(NodeEvent::VideoOutputAdded(
                self.node_id.clone(),
                video_output_id,
                channel.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        phaneron_plugin::traits::VideoOutput_TO::from_value(
            VideoOutput::new(self.inner.channel_semaphore_provider.clone(), channel),
            TD_Opaque,
        )
    }

    fn create_to_rgba(
        &self,
        video_format: &phaneron_plugin::VideoFormat,
        colour_spec: &ColourSpec,
        width: usize,
        height: usize,
    ) -> phaneron_plugin::types::ToRGBA {
        let reader = video_format.get_reader(width, height);
        phaneron_plugin::traits::ToRGBA_TO::from_value(
            ToRGBA::new(self.inner.compute_context.clone(), colour_spec, reader),
            TD_Opaque,
        )
    }

    fn create_from_rgba(
        &self,
        video_format: &phaneron_plugin::VideoFormat,
        colour_spec: &ColourSpec,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> phaneron_plugin::types::FromRGBA {
        let writer = video_format.get_writer(width, height, interlace);
        phaneron_plugin::traits::FromRGBA_TO::from_value(
            FromRGBA::new(self.inner.compute_context.clone(), colour_spec, writer),
            TD_Opaque,
        )
    }

    fn create_process_shader(
        &self,
        kernel: RStr<'_>,
        program_name: RStr<'_>,
    ) -> phaneron_plugin::types::ProcessShader {
        self.inner
            .compute_context
            .create_process_shader(kernel.into(), program_name.into())
    }

    fn create_to_audio_f32(
        &self,
        audio_format: phaneron_plugin::AudioFormat,
        channel_layout: AudioChannelLayout,
    ) -> phaneron_plugin::types::ToAudioF32 {
        phaneron_plugin::traits::ToAudioF32_TO::from_value(
            ToAudioF32::new(audio_format, channel_layout),
            TD_Opaque,
        )
    }

    fn create_from_audio_f32(
        &self,
        audio_format: phaneron_plugin::AudioFormat,
        channel_layout: AudioChannelLayout,
    ) -> phaneron_plugin::types::FromAudioF32 {
        phaneron_plugin::traits::FromAudioF32_TO::from_value(
            FromAudioF32::new(audio_format, channel_layout),
            TD_Opaque,
        )
    }
}

impl Clone for NodeContextImpl {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            inner: self.inner.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeStateEvent {
    StateChanged(NodeId, String),
    AudioInputAdded(NodeId, AudioInputId),
    VideoInputAdded(NodeId, VideoInputId),
    AudioOutputAdded(NodeId, AudioOutputId),
    VideoOutputAdded(NodeId, VideoOutputId),
}

#[derive(Debug, Clone)]
pub enum NodeEvent {
    AudioInputAdded(NodeId, AudioInputId),
    VideoInputAdded(NodeId, VideoInputId),
    AudioOutputAdded(
        NodeId,
        AudioOutputId,
        Channel<phaneron_plugin::types::AudioFrame>,
    ),
    VideoOutputAdded(
        NodeId,
        VideoOutputId,
        Channel<phaneron_plugin::types::VideoFrame>,
    ),
}

struct NodeContextInner {
    node_id: NodeId,
    compute_context: PhaneronComputeContext,
    event_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
    channel_semaphore_provider: ChannelSemaphoreProvider,
}

pub struct RunProcessFrameContext {
    pub audio_input_ids: Vec<AudioInputId>,
    pub audio_outputs: HashMap<AudioOutputId, Channel<phaneron_plugin::types::AudioFrame>>,
    pub video_input_ids: Vec<VideoInputId>,
    pub video_outputs: HashMap<VideoOutputId, Channel<phaneron_plugin::types::VideoFrame>>,
    pub connected_audio_pipes: Arc<Mutex<HashMap<AudioInputId, (AudioOutputId, AudioPipe)>>>,
    pub connected_video_pipes: Arc<Mutex<HashMap<VideoInputId, (VideoOutputId, VideoPipe)>>>,
}

impl RunProcessFrameContext {
    fn new(
        audio_input_ids: Vec<AudioInputId>,
        audio_outputs: HashMap<AudioOutputId, Channel<phaneron_plugin::types::AudioFrame>>,
        video_input_ids: Vec<VideoInputId>,
        video_outputs: HashMap<VideoOutputId, Channel<phaneron_plugin::types::VideoFrame>>,
        connected_audio_pipes: Arc<Mutex<HashMap<AudioInputId, (AudioOutputId, AudioPipe)>>>,
        connected_video_pipes: Arc<Mutex<HashMap<VideoInputId, (VideoOutputId, VideoPipe)>>>,
    ) -> Self {
        Self {
            audio_input_ids,
            audio_outputs,
            video_input_ids,
            video_outputs,
            connected_audio_pipes,
            connected_video_pipes,
        }
    }
}

#[derive(Default)]
pub struct ProcessFrameContextImpl {
    submitted: std::sync::Mutex<bool>,
}
impl phaneron_plugin::traits::ProcessFrameContext for ProcessFrameContextImpl {
    fn submit(&self) -> RResult<phaneron_plugin::types::FrameContext, RString> {
        if *self.submitted.lock().unwrap() {
            return RErr("Already submitted".to_string().into());
        }
        *self.submitted.lock().unwrap() = true;
        ROk(phaneron_plugin::traits::FrameContext_TO::from_value(
            FrameContextImpl {},
            TD_Opaque,
        ))
    }
}

pub struct FrameContextImpl {}
impl phaneron_plugin::traits::FrameContext for FrameContextImpl {}

#[derive(Debug)]
pub enum AudioConnectionError {
    InputDoesNotExist(AudioInputId),
    InputAlreadyConnectedTo(AudioInputId, AudioOutputId),
}

#[derive(Debug)]
pub enum VideoConnectionError {
    InputDoesNotExist(VideoInputId),
    InputAlreadyConnectedTo(VideoInputId, VideoOutputId),
}

pub async fn create_node_context(
    context: PhaneronComputeContext,
    node_id: NodeId,
    state_tx: UnboundedSender<NodeStateEvent>,
) -> (
    phaneron_plugin::types::NodeContext,
    NodeRunContext,
    UnboundedReceiver<NodeEvent>,
    ChannelSemaphoreProvider,
) {
    let node_run_context = NodeRunContext::new(node_id.clone(), state_tx);
    let (node_event_tx, node_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let node_semaphore_provider = ChannelSemaphoreProvider::default();
    let node_context = NodeContextImpl::new(
        node_id,
        context,
        node_event_tx,
        node_semaphore_provider.clone(),
    );
    let node_context = RArc::new(phaneron_plugin::traits::NodeContext_TO::from_value(
        node_context,
        TD_Opaque,
    ));

    (
        node_context,
        node_run_context,
        node_event_rx,
        node_semaphore_provider,
    )
}

pub async fn run_node(
    context: PhaneronComputeContext,
    node_context: NodeRunContext,
    node: Arc<phaneron_plugin::types::Node>,
    pending_state: Arc<tokio::sync::Mutex<Option<String>>>,
    node_state_event_tx: tokio::sync::mpsc::UnboundedSender<NodeStateEvent>,
    mut node_event_rx: tokio::sync::mpsc::UnboundedReceiver<NodeEvent>,
    semaphore_provider: ChannelSemaphoreProvider,
) {
    let mut previous_black_frame: Option<(usize, usize, VideoFrameWithId)> = None;
    let mut previous_silence_frame: Option<AudioFrameWithId> = None;
    loop {
        let run_node_context = node_context.get_run_process_frame_context().await;
        if !run_node_context.video_input_ids.is_empty()
            && run_node_context.connected_video_pipes.lock().await.len()
                != run_node_context.video_input_ids.len()
        {
            // No connections, can't make progress
            while let Ok(event) = node_event_rx.try_recv() {
                handle_node_event(event, node_context.clone()).await;
            }
            continue;
        }

        if !run_node_context.audio_input_ids.is_empty()
            && run_node_context.connected_audio_pipes.lock().await.len()
                != run_node_context.audio_input_ids.len()
        {
            // No connections, can't make progress
            while let Ok(event) = node_event_rx.try_recv() {
                handle_node_event(event, node_context.clone()).await;
            }
            continue;
        }

        let mut no_connections = false;
        {
            for output in run_node_context.video_outputs.iter() {
                no_connections |= output.1.no_receivers().await;
            }
        }

        if no_connections {
            // No connections, can't make progress
            while let Ok(event) = node_event_rx.try_recv() {
                handle_node_event(event, node_context.clone()).await;
            }
            continue;
        }

        let mut no_connections = false;
        {
            for output in run_node_context.audio_outputs.iter() {
                no_connections |= output.1.no_receivers().await;
            }
        }

        if no_connections {
            // No connections, can't make progress
            while let Ok(event) = node_event_rx.try_recv() {
                handle_node_event(event, node_context.clone()).await;
            }
            continue;
        }

        if let Some(state) = pending_state.lock().await.take() {
            apply_node_state(
                node_context.node_id.clone(),
                node.clone(),
                state,
                node_state_event_tx.clone(),
            )
            .await;
        }

        let mut audio_frames: HashMap<AudioInputId, AudioFrameWithId> = HashMap::new();
        let mut video_frames: HashMap<VideoInputId, VideoFrameWithId> = HashMap::new();

        let mut inputs_requiring_silence: Vec<AudioInputId> = vec![];
        let mut inputs_requiring_black_frames: Vec<VideoInputId> = vec![];
        let mut max_width = 256;
        let mut max_height = 1;

        let mut upstream_semaphores: Vec<ChannelSemaphore> = vec![];

        for input_id in run_node_context.audio_input_ids.clone() {
            let mut audio_pipes_lock = run_node_context.connected_audio_pipes.lock().await;
            match audio_pipes_lock.get_mut(&input_id) {
                Some((pipe_id, pipe)) => match pipe.next_frame().await {
                    Some((frame, semaphore)) => {
                        upstream_semaphores.push(semaphore);
                        audio_frames
                            .insert(input_id, AudioFrameWithId::new(pipe_id.clone(), frame));
                    }
                    None => {
                        inputs_requiring_silence.push(input_id.clone());
                        todo!("Tell context to disconnect pipe");
                    }
                },
                None => inputs_requiring_silence.push(input_id.clone()),
            }
        }

        for input_id in run_node_context.video_input_ids.clone() {
            let mut video_pipes_lock = run_node_context.connected_video_pipes.lock().await;
            match video_pipes_lock.get_mut(&input_id) {
                Some((pipe_id, pipe)) => match pipe.next_frame().await {
                    Some((frame, semaphore)) => {
                        upstream_semaphores.push(semaphore);
                        max_width = max_width.max(frame.width());
                        max_height = max_height.max(frame.height());
                        video_frames
                            .insert(input_id, VideoFrameWithId::new(pipe_id.clone(), frame));
                    }
                    None => {
                        inputs_requiring_black_frames.push(input_id);
                        todo!("Tell context to disconnect pipe");
                    }
                },
                None => {
                    inputs_requiring_black_frames.push(input_id);
                }
            }
        }

        let black_frame = match previous_black_frame.take() {
            Some((width, height, frame)) => {
                if max_width > width || max_height > height {
                    let frame = context.create_black_frame(max_width, max_height);
                    let frame = RArc::new(phaneron_plugin::traits::VideoFrame_TO::from_value(
                        frame, TD_Opaque,
                    ));
                    VideoFrameWithId::new(VideoOutputId::new_from("black".into()), frame)
                } else {
                    frame
                }
            }
            None => {
                let frame = context.create_black_frame(max_width, max_height);
                let frame = RArc::new(phaneron_plugin::traits::VideoFrame_TO::from_value(
                    frame, TD_Opaque,
                ));
                VideoFrameWithId::new(VideoOutputId::new_from("black".into()), frame)
            }
        };

        let silence_frame = match previous_silence_frame.take() {
            Some(frame) => frame,
            None => {
                let frame = AudioFrame::new(
                    AudioFrameId::new_from("silence".to_string()),
                    vec![vec![0f32; 48000 / 25]],
                ); // TODO: Framerate, number of samples, frames
                let frame = phaneron_plugin::traits::AudioFrame_TO::from_value(frame, TD_Opaque);
                AudioFrameWithId::new(AudioOutputId::new_from("silence".into()), RArc::new(frame))
            }
        };

        if !inputs_requiring_black_frames.is_empty() {
            for input_id in inputs_requiring_black_frames.iter() {
                video_frames.insert(input_id.clone(), black_frame.clone());
            }
        }

        if !inputs_requiring_silence.is_empty() {
            for input_id in inputs_requiring_silence.iter() {
                audio_frames.insert(input_id.clone(), silence_frame.clone());
            }
        }

        {
            let node = node.clone();
            let silence = silence_frame.clone();
            let black = black_frame.clone();
            let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
            std::thread::spawn(move || {
                node.process_frame(
                    phaneron_plugin::traits::ProcessFrameContext_TO::from_value(
                        ProcessFrameContextImpl::default(),
                        TD_Opaque,
                    ),
                    video_frames.into(),
                    audio_frames.into(),
                    black,
                    silence,
                );
                sender.blocking_send(()).unwrap();
            });
            receiver.recv().await;
        }

        let _ = previous_black_frame.insert((max_width, max_height, black_frame));
        let _ = previous_silence_frame.insert(silence_frame);

        let downstream_semaphores = semaphore_provider.drain();

        for semaphore in downstream_semaphores {
            semaphore.await.ok();
        }

        for semaphore in upstream_semaphores {
            semaphore.signal().await
        }

        while let Ok(event) = node_event_rx.try_recv() {
            handle_node_event(event, node_context.clone()).await;
        }
    }
}

pub async fn handle_node_event(event: NodeEvent, node_context: NodeRunContext) {
    match event {
        NodeEvent::AudioInputAdded(_, audio_input_id) => {
            node_context.add_audio_input(audio_input_id).await;
        }
        NodeEvent::VideoInputAdded(_, video_input_id) => {
            node_context.add_video_input(video_input_id).await;
        }
        NodeEvent::AudioOutputAdded(_, audio_output_id, channel) => {
            node_context
                .add_audio_output(audio_output_id, channel)
                .await;
        }
        NodeEvent::VideoOutputAdded(_, video_output_id, channel) => {
            node_context
                .add_video_output(video_output_id, channel)
                .await
        }
    }
}

pub async fn apply_node_state(
    node_id: NodeId,
    node: Arc<phaneron_plugin::types::Node>,
    state: String,
    node_state_event_tx: tokio::sync::mpsc::UnboundedSender<NodeStateEvent>,
) {
    let node_state = state.clone();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    std::thread::spawn(move || {
        let applied = node.apply_state(node_state.into());
        sender.blocking_send(applied).unwrap();
    });
    let applied = receiver.recv().await.unwrap();
    if applied {
        node_state_event_tx
            .send(NodeStateEvent::StateChanged(node_id, state))
            .ok();
    } else {
        // TODO: Event
    }
}
