use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    channel::{Channel, ChannelSemaphore},
    colour::ColourSpace,
    compute::{
        audio_frame::{AudioFrame, AudioFrameId},
        audio_output::{AudioOutput, AudioPipe},
        video_frame::VideoFrame,
        video_output::{VideoOutput, VideoPipe},
        PhaneronComputeContext,
    },
    format::VideoFormat,
    graph::{AudioInputId, AudioOutputId, NodeId, VideoInputId, VideoOutputId},
    io::{FromRGBA, InterlaceMode, ToRGBA},
};

pub struct NodeContext {
    pub node_id: NodeId,
    inner: Arc<NodeContextInner>,
}

impl NodeContext {
    pub fn new(
        node_id: NodeId,
        context: PhaneronComputeContext,
        state_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
    ) -> Self {
        Self {
            node_id: node_id.clone(),
            inner: Arc::new(NodeContextInner::new(node_id, context, state_tx)),
        }
    }

    pub async fn set_state(&self, state: String) {
        self.inner.pending_state.lock().await.replace(state);
    }

    pub fn get_pending_state_channel(&self) -> Arc<tokio::sync::Mutex<Option<String>>> {
        self.inner.pending_state.clone()
    }

    pub fn get_run_node_context(&self) -> RunNodeContext {
        let connected_audio_pipes = self.inner.connected_audio_pipes.clone();
        let connected_video_pipes = self.inner.connected_video_pipes.clone();
        let audio_input_ids = self.inner.audio_input_ids.clone();
        let video_input_ids = self.inner.video_input_ids.clone();
        let node_semaphores = self.inner.semaphores.clone();
        let video_outputs = self.inner.video_outputs.clone();
        let audio_outputs = self.inner.audio_outputs.clone();

        RunNodeContext::new(
            audio_input_ids,
            audio_outputs,
            video_input_ids,
            video_outputs,
            node_semaphores,
            connected_audio_pipes,
            connected_video_pipes,
        )
    }

    pub async fn add_audio_input(
        &self,
        audio_input_id: AudioInputId,
    ) -> Result<(), AddAudioInputError> {
        let mut audio_input_ids = self.inner.audio_input_ids.lock().await;
        if audio_input_ids.contains(&audio_input_id) {
            Err(AddAudioInputError::InputAlreadyExists)
        } else {
            audio_input_ids.push(audio_input_id.clone());
            self.inner
                .state_tx
                .send(NodeEvent::AudioInputAdded(
                    self.node_id.clone(),
                    audio_input_id.clone(),
                ))
                .ok(); // If receiver is dropped, not much we can do
            Ok(())
        }
    }

    pub async fn add_video_input(
        &self,
        video_input_id: VideoInputId,
    ) -> Result<(), AddVideoInputError> {
        let mut video_input_ids = self.inner.video_input_ids.lock().await;
        if video_input_ids.contains(&video_input_id) {
            Err(AddVideoInputError::InputAlreadyExists)
        } else {
            video_input_ids.push(video_input_id.clone());
            self.inner
                .state_tx
                .send(NodeEvent::VideoInputAdded(
                    self.node_id.clone(),
                    video_input_id.clone(),
                ))
                .ok(); // If receiver is dropped, not much we can do
            Ok(())
        }
    }

    pub async fn add_audio_output(&self) -> AudioOutput {
        let channel = Channel::default();
        let audio_output_id = AudioOutputId::default();
        self.inner
            .audio_outputs
            .lock()
            .await
            .insert(audio_output_id.clone(), channel.clone());
        self.inner
            .state_tx
            .send(NodeEvent::AudioOutputAdded(
                self.node_id.clone(),
                audio_output_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        AudioOutput::new(
            NodeContext {
                node_id: self.node_id.clone(),
                inner: self.inner.clone(),
            },
            channel,
        )
    }

    pub async fn add_video_output(&self) -> VideoOutput {
        let channel = Channel::default();
        let video_output_id = VideoOutputId::default();
        self.inner
            .video_outputs
            .lock()
            .await
            .insert(video_output_id.clone(), channel.clone());
        self.inner
            .state_tx
            .send(NodeEvent::VideoOutputAdded(
                self.node_id.clone(),
                video_output_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        VideoOutput::new(
            NodeContext {
                node_id: self.node_id.clone(),
                inner: self.inner.clone(),
            },
            channel,
        )
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

    pub async fn connect_video_pipe(
        &self,
        to_video_input: &VideoInputId,
        video_pipe: VideoPipe,
    ) -> Result<(), VideoConnectionError> {
        self.inner
            .connect_video_pipe(to_video_input, &video_pipe.id.clone(), video_pipe)
            .await
    }

    pub async fn connect_audio_pipe(
        &self,
        to_audio_input: &AudioInputId,
        audio_pipe: AudioPipe,
    ) -> Result<(), AudioConnectionError> {
        self.inner
            .connect_audio_pipe(to_audio_input, &audio_pipe.id.clone(), audio_pipe)
            .await
    }

    pub async fn get_frame_semaphore(&self) -> ChannelSemaphore {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let mut semaphores = self.inner.semaphores.lock().await;
        semaphores.push(receiver);

        ChannelSemaphore::new(sender)
    }

    pub fn create_to_rgba(
        &self,
        video_format: &VideoFormat,
        colour_space: &ColourSpace,
        width: usize,
        height: usize,
    ) -> ToRGBA {
        self.inner
            .create_to_rgba(video_format, colour_space, width, height)
    }

    pub fn create_from_rgba(
        &self,
        video_format: &VideoFormat,
        colour_space: &ColourSpace,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> FromRGBA {
        self.inner
            .create_from_rgba(video_format, colour_space, width, height, interlace)
    }

    #[deprecated]
    pub fn create_process_shader(
        &self,
        kernel: &str,
        program_name: &str,
    ) -> opencl3::kernel::Kernel {
        self.inner
            .context
            .create_process_shader(kernel, program_name)
    }

    #[deprecated]
    pub fn create_video_frame_buffer(
        &self,
        num_bytes_rgba: usize,
    ) -> opencl3::memory::Buffer<opencl3::types::cl_uchar> {
        self.inner.context.create_video_frame_buffer(num_bytes_rgba)
    }

    pub fn run_process_shader(&self, execute_kernel: opencl3::kernel::ExecuteKernel<'_>) {
        self.inner.context.run_process_shader(execute_kernel)
    }

    #[deprecated]
    pub fn create_image(&self, width: usize, height: usize) -> opencl3::memory::Image {
        self.inner.context.create_image(width, height)
    }

    #[deprecated]
    pub fn create_image_from_buffer(
        &self,
        width: usize,
        height: usize,
        buffer: &opencl3::memory::Buffer<opencl3::types::cl_uchar>,
    ) -> opencl3::memory::Image {
        self.inner
            .context
            .create_image_from_buffer(width, height, buffer)
    }
}

impl Clone for NodeContext {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            inner: self.inner.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum NodeEvent {
    StateChanged(NodeId, String),
    AudioInputAdded(NodeId, AudioInputId),
    VideoInputAdded(NodeId, VideoInputId),
    AudioOutputAdded(NodeId, AudioOutputId),
    VideoOutputAdded(NodeId, VideoOutputId),
    VideoPipeConnected(NodeId, VideoInputId, VideoOutputId),
    AudioPipeConnected(NodeId, AudioInputId, AudioOutputId),
}

struct NodeContextInner {
    node_id: NodeId,
    context: PhaneronComputeContext,
    state_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
    audio_input_ids: Arc<Mutex<Vec<AudioInputId>>>,
    audio_outputs: Arc<Mutex<HashMap<AudioOutputId, Channel<AudioFrame>>>>,
    video_input_ids: Arc<Mutex<Vec<VideoInputId>>>,
    video_outputs: Arc<Mutex<HashMap<VideoOutputId, Channel<VideoFrame>>>>,
    connected_audio_pipes: Arc<Mutex<HashMap<AudioInputId, (AudioOutputId, AudioPipe)>>>,
    connected_video_pipes: Arc<Mutex<HashMap<VideoInputId, (VideoOutputId, VideoPipe)>>>,
    semaphores: Arc<Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>>,
    pending_state: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl NodeContextInner {
    pub fn new(
        node_id: NodeId,
        context: PhaneronComputeContext,
        state_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
    ) -> Self {
        Self {
            node_id,
            context,
            state_tx,
            audio_input_ids: Default::default(),
            audio_outputs: Default::default(),
            video_input_ids: Default::default(),
            video_outputs: Default::default(),
            connected_audio_pipes: Default::default(),
            connected_video_pipes: Default::default(),
            semaphores: Default::default(),
            pending_state: Default::default(),
        }
    }

    pub async fn connect_video_pipe(
        &self,
        to_video_input: &VideoInputId,
        video_pipe_id: &VideoOutputId,
        video_pipe: VideoPipe,
    ) -> Result<(), VideoConnectionError> {
        let video_input_ids = self.video_input_ids.lock().await;
        if !video_input_ids.contains(to_video_input) {
            return Err(VideoConnectionError::InputDoesNotExist(
                to_video_input.clone(),
            ));
        }

        let mut connected_video_pipes = self.connected_video_pipes.lock().await;
        if let Some(conn) = connected_video_pipes.get(to_video_input) {
            return Err(VideoConnectionError::InputAlreadyConnectedTo(
                to_video_input.clone(),
                conn.0.clone(),
            ));
        }

        connected_video_pipes.insert(to_video_input.clone(), (video_pipe_id.clone(), video_pipe));
        self.state_tx
            .send(NodeEvent::VideoPipeConnected(
                self.node_id.clone(),
                to_video_input.clone(),
                video_pipe_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        Ok(())
    }

    pub async fn connect_audio_pipe(
        &self,
        to_audio_input: &AudioInputId,
        audio_pipe_id: &AudioOutputId,
        audio_pipe: AudioPipe,
    ) -> Result<(), AudioConnectionError> {
        let audio_input_ids = self.audio_input_ids.lock().await;
        if !audio_input_ids.contains(to_audio_input) {
            return Err(AudioConnectionError::InputDoesNotExist(
                to_audio_input.clone(),
            ));
        }

        let mut connected_audio_pipes = self.connected_audio_pipes.lock().await;
        if let Some(conn) = connected_audio_pipes.get(to_audio_input) {
            return Err(AudioConnectionError::InputAlreadyConnectedTo(
                to_audio_input.clone(),
                conn.0.clone(),
            ));
        }

        connected_audio_pipes.insert(to_audio_input.clone(), (audio_pipe_id.clone(), audio_pipe));
        self.state_tx
            .send(NodeEvent::AudioPipeConnected(
                self.node_id.clone(),
                to_audio_input.clone(),
                audio_pipe_id.clone(),
            ))
            .ok(); // If receiver is dropped, not much we can do

        Ok(())
    }

    fn create_to_rgba(
        &self,
        video_format: &VideoFormat,
        colour_space: &ColourSpace,
        width: usize,
        height: usize,
    ) -> ToRGBA {
        let reader = video_format.get_reader(width, height);
        ToRGBA::new(self.context.clone(), colour_space.colour_spec(), reader)
    }

    fn create_from_rgba(
        &self,
        video_format: &VideoFormat,
        colour_space: &ColourSpace,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> FromRGBA {
        let writer = video_format.get_writer(width, height, interlace);
        FromRGBA::new(self.context.clone(), colour_space.colour_spec(), writer)
    }
}

pub struct RunNodeContext {
    pub audio_input_ids: Arc<Mutex<Vec<AudioInputId>>>,
    pub audio_outputs: Arc<Mutex<HashMap<AudioOutputId, Channel<AudioFrame>>>>,
    pub video_input_ids: Arc<Mutex<Vec<VideoInputId>>>,
    pub video_outputs: Arc<Mutex<HashMap<VideoOutputId, Channel<VideoFrame>>>>,
    pub node_semaphores: Arc<Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>>,
    pub connected_audio_pipes: Arc<Mutex<HashMap<AudioInputId, (AudioOutputId, AudioPipe)>>>,
    pub connected_video_pipes: Arc<Mutex<HashMap<VideoInputId, (VideoOutputId, VideoPipe)>>>,
}

impl RunNodeContext {
    fn new(
        audio_input_ids: Arc<Mutex<Vec<AudioInputId>>>,
        audio_outputs: Arc<Mutex<HashMap<AudioOutputId, Channel<AudioFrame>>>>,
        video_input_ids: Arc<Mutex<Vec<VideoInputId>>>,
        video_outputs: Arc<Mutex<HashMap<VideoOutputId, Channel<VideoFrame>>>>,
        node_semaphores: Arc<Mutex<Vec<tokio::sync::oneshot::Receiver<()>>>>,
        connected_audio_pipes: Arc<Mutex<HashMap<AudioInputId, (AudioOutputId, AudioPipe)>>>,
        connected_video_pipes: Arc<Mutex<HashMap<VideoInputId, (VideoOutputId, VideoPipe)>>>,
    ) -> Self {
        Self {
            audio_input_ids,
            audio_outputs,
            video_input_ids,
            video_outputs,
            node_semaphores,
            connected_audio_pipes,
            connected_video_pipes,
        }
    }
}

pub struct ProcessFrameContext {
    context: PhaneronComputeContext,
    node_id: NodeId,
}
impl ProcessFrameContext {
    pub fn new(context: PhaneronComputeContext, node_id: NodeId) -> Self {
        Self { node_id, context }
    }

    pub async fn submit(self) -> FrameContext {
        let context = self.context.clone();
        let node_id = self.node_id.clone();

        drop(self);

        FrameContext { context, node_id }
    }
}

pub struct FrameContext {
    context: PhaneronComputeContext,
    node_id: NodeId,
}

impl FrameContext {}

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

#[derive(Debug)]
pub enum AddAudioInputError {
    InputAlreadyExists,
}

#[derive(Debug)]
pub enum AddVideoInputError {
    InputAlreadyExists,
}

#[async_trait]
pub trait Node: Send + Sync {
    async fn apply_state(&self, state: String) -> bool;
    async fn process_frame(
        &self,
        frame_context: ProcessFrameContext,
        video_frames: HashMap<VideoInputId, (VideoOutputId, VideoFrame)>,
        audio_frames: HashMap<AudioInputId, (AudioOutputId, AudioFrame)>,
        black_frame: (VideoOutputId, VideoFrame),
        silence_frame: (AudioOutputId, AudioFrame),
    );
}

pub async fn run_node(
    context: PhaneronComputeContext,
    node_context: NodeContext,
    node: Box<dyn Node>,
    pending_state: Arc<tokio::sync::Mutex<Option<String>>>,
    node_event_tx: tokio::sync::mpsc::UnboundedSender<NodeEvent>,
) {
    let mut previous_black_frame: Option<(usize, usize, VideoFrame)> = None;
    let mut previous_silence_frame: Option<AudioFrame> = None;
    loop {
        let run_node_context = node_context.get_run_node_context();
        if run_node_context.video_input_ids.lock().await.len() > 0
            && run_node_context.connected_video_pipes.lock().await.len() == 0
        {
            // No connections, can't make progress
            continue;
        }

        let mut no_connections = false;
        {
            let outputs_lock = run_node_context.video_outputs.lock().await;
            for output in outputs_lock.iter() {
                no_connections |= output.1.no_receivers().await;
            }
        }

        if no_connections {
            // No connections, can't make progress
            continue;
        }

        if let Some(state) = pending_state.lock().await.take() {
            let applied = node.apply_state(state.clone()).await;
            if applied {
                node_event_tx
                    .send(NodeEvent::StateChanged(node_context.node_id.clone(), state))
                    .ok();
            } else {
                // TODO: Event
            }
        }

        let mut audio_pipes_lock = run_node_context.connected_audio_pipes.lock().await;
        let mut video_pipes_lock = run_node_context.connected_video_pipes.lock().await;
        let mut audio_frames = HashMap::with_capacity(audio_pipes_lock.len());
        let mut video_frames = HashMap::with_capacity(video_pipes_lock.len());

        let mut inputs_requiring_silence: Vec<AudioInputId> = vec![];
        let mut inputs_requiring_black_frames: Vec<VideoInputId> = vec![];
        let mut max_width = 256;
        let mut max_height = 1;

        let mut downstream_semaphores: Vec<ChannelSemaphore> = vec![];

        for input_id in run_node_context.audio_input_ids.lock().await.clone() {
            match audio_pipes_lock.get_mut(&input_id) {
                Some((pipe_id, pipe)) => match pipe.next_frame().await {
                    Some((frame, semaphore)) => {
                        downstream_semaphores.push(semaphore);
                        audio_frames.insert(input_id, (pipe_id.clone(), frame));
                    }
                    None => {
                        inputs_requiring_silence.push(input_id);
                        todo!("Tell context to disconnect pipe");
                    }
                },
                None => inputs_requiring_silence.push(input_id),
            }
        }

        for input_id in run_node_context.video_input_ids.lock().await.clone() {
            match video_pipes_lock.get_mut(&input_id) {
                Some((pipe_id, pipe)) => match pipe.next_frame().await {
                    Some((frame, semaphore)) => {
                        downstream_semaphores.push(semaphore);
                        max_width = max_width.max(frame.width());
                        max_height = max_height.max(frame.height());
                        video_frames.insert(input_id, (pipe_id.clone(), frame));
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
                    context.create_black_frame(max_width, max_height)
                } else {
                    frame
                }
            }
            None => context.create_black_frame(max_width, max_height),
        };

        let silence_frame = match previous_silence_frame.take() {
            Some(frame) => frame,
            None => {
                AudioFrame::new(
                    AudioFrameId::new_from("silence".to_string()),
                    vec![vec![0f32; 48000 / 25]],
                ) // TODO: Framerate, number of samples, frames
            }
        };

        if !inputs_requiring_black_frames.is_empty() {
            for input_id in inputs_requiring_black_frames {
                video_frames.insert(
                    input_id,
                    (
                        VideoOutputId::new_from("black".to_string()),
                        black_frame.clone(),
                    ),
                );
            }
        }

        let fut = node.process_frame(
            ProcessFrameContext::new(context.clone(), node_context.node_id.clone()),
            video_frames,
            audio_frames,
            (
                VideoOutputId::new_from("black".to_string()),
                black_frame.clone(),
            ),
            (
                AudioOutputId::new_from("silence".to_string()),
                silence_frame.clone(),
            ),
        );
        fut.await;

        let _ = previous_black_frame.insert((max_width, max_height, black_frame));
        let _ = previous_silence_frame.insert(silence_frame);

        let upstream_semaphores: Vec<tokio::sync::oneshot::Receiver<()>> = {
            let mut lock = run_node_context.node_semaphores.lock().await;
            lock.drain(..).collect()
        };

        for semaphore in upstream_semaphores {
            semaphore.await.ok();
        }

        for semaphore in downstream_semaphores {
            semaphore.signal().await
        }
    }
}
