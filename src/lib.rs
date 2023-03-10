#[macro_use]
extern crate lazy_static;

pub use crate::api::initialize_api;
pub use crate::compute::{audio_stream::AudioPipe, create_compute_context};
pub use crate::graph::{AudioInputId, GraphId, NodeId, VideoInputId};
pub use crate::node_context::NodeContext;
pub use plugins::ffmpeg_producer::FFmpegProducerPlugin;
pub use plugins::traditional_mixer_emulator::TraditionalMixerEmulatorPlugin;
pub use plugins::turbo_consumer::TurboConsumerPlugin;
pub use plugins::webrtc_consumer::WebRTCConsumerPlugin;
pub use state::PhaneronState;

// TODO: Remove
pub use plugins::ffmpeg_producer::FFmpegProducerConfiguration;
pub use plugins::traditional_mixer_emulator::{
    TraditionalMixerEmulatorConfiguration, TraditionalMixerEmulatorState,
};

mod api;
mod channel;
mod colour;
mod compute;
mod dissolve;
mod format;
mod graph;
mod io;
mod load_save;
mod node_context;
mod plugins;
mod state;
mod yadif;
