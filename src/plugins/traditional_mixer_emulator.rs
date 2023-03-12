use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    compute::{audio_frame::AudioFrame, video_frame::VideoFrame, video_output::VideoOutput},
    dissolve::Dissolve,
    graph::{AudioInputId, AudioOutputId, NodeId, VideoInputId, VideoOutputId},
    node_context::{Node, NodeContext, ProcessFrameContext},
};

pub struct TraditionalMixerEmulatorPlugin {}
impl TraditionalMixerEmulatorPlugin {
    pub fn load() -> Self {
        Self {}
    }

    pub async fn initialize(&mut self) {
        info!("Traditional Mixer Emulator plugin initializing");
        info!("Traditional Mixer Emulator plugin initialized");
    }

    pub async fn create_node(&mut self, node_id: NodeId) -> TraditionalMixerEmlator {
        TraditionalMixerEmlator::new(node_id)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraditionalMixerEmulatorState {
    pub active_input: Option<String>,
    pub next_input: Option<String>,
    pub transition: Option<TraditionalMixerEmulatorTransition>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "transition")]
pub enum TraditionalMixerEmulatorTransition {
    Mix { position: f32 },
}

pub struct TraditionalMixerEmulatorConfiguration {
    pub number_of_inputs: usize,
}

pub struct TraditionalMixerEmlator {
    node_id: NodeId,
    context: Mutex<Option<NodeContext>>,
    state: Mutex<Option<TraditionalMixerEmulatorState>>,
    active_video_output: Mutex<Option<VideoOutput>>,
    video_transition: Mutex<Option<Dissolve>>,
}

impl TraditionalMixerEmlator {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            context: Default::default(),
            state: Default::default(),
            active_video_output: Default::default(),
            video_transition: Default::default(),
        }
    }

    pub async fn initialize(
        &mut self,
        context: NodeContext,
        configuration: TraditionalMixerEmulatorConfiguration,
    ) {
        info!(
            "Traditional Mixer Emulator {} initializing with {} inputs",
            self.node_id, configuration.number_of_inputs
        );

        let mut input_ids: Vec<VideoInputId> = Vec::with_capacity(configuration.number_of_inputs);
        for _ in 0..configuration.number_of_inputs {
            input_ids.push(VideoInputId::default());
        }
        let input_ids = Arc::new(Mutex::new(input_ids));

        for id in &*input_ids.lock().await {
            context.add_video_input(id.clone()).await.unwrap();
        }

        let active_video_output = context.add_video_output().await;
        self.active_video_output
            .lock()
            .await
            .replace(active_video_output);

        self.context.lock().await.replace(context);

        info!("Traditional Mixer Emulator {} initialized", self.node_id);
    }
}

#[async_trait]
impl Node for TraditionalMixerEmlator {
    async fn apply_state(&self, state: String) -> bool {
        let state: TraditionalMixerEmulatorState = serde_json::from_str(&state).unwrap();
        self.state.lock().await.replace(state);

        true
    }
    async fn process_frame(
        &self,
        frame_context: ProcessFrameContext,
        video_frames: HashMap<VideoInputId, (VideoOutputId, VideoFrame)>,
        audio_frames: HashMap<AudioInputId, (AudioOutputId, AudioFrame)>,
        black_frame: (VideoOutputId, VideoFrame),
        silence_frame: (AudioOutputId, AudioFrame),
    ) {
        let state = self.state.lock().await;
        let (active_input, next_input) = if let Some(state) = &*state {
            (
                state
                    .active_input
                    .clone()
                    .unwrap_or_else(|| black_frame.0.clone().to_string()),
                state
                    .next_input
                    .clone()
                    .unwrap_or_else(|| black_frame.0.clone().to_string()),
            )
        } else {
            (
                black_frame.0.clone().to_string(),
                black_frame.0.clone().to_string(),
            )
        };

        let active_input = VideoInputId::new_from(active_input);
        let next_input = VideoInputId::new_from(next_input);
        let active_input = video_frames.get(&active_input).unwrap_or(&black_frame);
        let next_input = video_frames.get(&next_input).unwrap_or(&black_frame);

        let output = if let Some(state) = &*state {
            if let Some(transition) = &state.transition {
                match transition {
                    TraditionalMixerEmulatorTransition::Mix { position } => {
                        let node_context = self.context.lock().await;
                        let mut video_transition_lock = self.video_transition.lock().await;
                        let video_transition = video_transition_lock.get_or_insert_with(|| {
                            Dissolve::new(node_context.as_ref().unwrap(), 1920, 1080)
                        });
                        let output =
                            video_transition.run(&active_input.1, &next_input.1, *position);
                        output.first().unwrap().clone()
                    }
                }
            } else {
                active_input.1.clone()
            }
        } else {
            active_input.1.clone()
        };

        let frame_context = frame_context.submit().await;
        self.active_video_output
            .lock()
            .await
            .as_mut()
            .unwrap()
            .push_frame(&frame_context, output.clone())
            .await;
    }
}
