use std::sync::Mutex;

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{RHashMap, ROption, RString},
};
use serde::{Deserialize, Serialize};

use phaneron_plugin::{
    traits::Node_TO, types::Node, types::NodeContext, types::ProcessFrameContext,
    types::VideoOutput, AudioFrameWithId, AudioInputId, VideoFrameWithId, VideoInputId,
};

use crate::dissolve::Dissolve;

pub struct TraditionalMixerEmulatorHandle {
    node_id: String,
}
impl TraditionalMixerEmulatorHandle {
    pub(super) fn new(node_id: String) -> Self {
        Self { node_id }
    }
}
impl phaneron_plugin::traits::NodeHandle for TraditionalMixerEmulatorHandle {
    fn initialize(&self, context: NodeContext, configuration: ROption<RString>) -> Node {
        let configuration = configuration.map::<String, _>(Into::<String>::into);
        let configuration = match configuration {
            ROption::RSome(config) => serde_json::from_str(&config).unwrap(),
            ROption::RNone => None,
        };
        let node = TraditionalMixerEmlator::new(self.node_id.clone(), context, configuration);

        Node_TO::from_value(node, TD_Opaque)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraditionalMixerEmulatorConfiguration {
    pub number_of_inputs: usize,
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

pub struct TraditionalMixerEmlator {
    node_id: String,
    context: NodeContext,
    state: Mutex<Option<TraditionalMixerEmulatorState>>,
    active_video_output: VideoOutput,
    video_transition: Mutex<Option<Dissolve>>,
}

impl TraditionalMixerEmlator {
    pub fn new(
        node_id: String,
        context: NodeContext,
        configuration: Option<TraditionalMixerEmulatorConfiguration>,
    ) -> Self {
        let active_video_output = context.add_video_output();

        if let Some(configuration) = configuration {
            for _ in 0..configuration.number_of_inputs {
                context.add_video_input();
            }
        }

        Self {
            node_id,
            context,
            active_video_output,
            state: Default::default(),
            video_transition: Default::default(),
        }
    }
}

impl phaneron_plugin::traits::Node for TraditionalMixerEmlator {
    fn apply_state(&self, state: RString) -> bool {
        let state: TraditionalMixerEmulatorState = serde_json::from_str(&state).unwrap();
        let mut state_lock = self.state.lock().unwrap();
        state_lock.replace(state);

        true
    }
    fn process_frame(
        &self,
        frame_context: ProcessFrameContext,
        video_frames: RHashMap<VideoInputId, VideoFrameWithId>,
        audio_frames: RHashMap<AudioInputId, AudioFrameWithId>,
        black_frame: VideoFrameWithId,
        silence_frame: AudioFrameWithId,
    ) {
        let state = self.state.lock().unwrap();
        let (active_input, next_input) = if let Some(state) = &*state {
            (
                state
                    .active_input
                    .clone()
                    .map(|id| VideoInputId::new_from(id.into())),
                state
                    .next_input
                    .clone()
                    .map(|id| VideoInputId::new_from(id.into())),
            )
        } else {
            (None, None)
        };

        let active_input = if let Some(input_id) = active_input {
            video_frames.get(&input_id).unwrap_or(&black_frame)
        } else {
            &black_frame
        };

        let next_input = if let Some(input_id) = next_input {
            video_frames.get(&input_id).unwrap_or(&black_frame)
        } else {
            &black_frame
        };

        let output = if let Some(state) = &*state {
            if let Some(transition) = &state.transition {
                match transition {
                    TraditionalMixerEmulatorTransition::Mix { position } => {
                        let mut video_transition_lock = self.video_transition.lock().unwrap();
                        let video_transition = video_transition_lock
                            .get_or_insert_with(|| Dissolve::new(&self.context, 1920, 1080));
                        let output =
                            video_transition.run(&active_input.frame, &next_input.frame, *position);
                        output.first().unwrap().clone()
                    }
                }
            } else {
                active_input.frame.clone()
            }
        } else {
            active_input.frame.clone()
        };

        let frame_context = frame_context.submit().unwrap();
        self.active_video_output.push_frame(&frame_context, output);
    }
}
