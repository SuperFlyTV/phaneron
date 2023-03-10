use std::fs;

use phaneron::{
    AudioPipe, FFmpegProducerConfiguration, NodeContext, PhaneronState,
    TraditionalMixerEmulatorConfiguration, TraditionalMixerEmulatorState,
};
use serde::Deserialize;
use tracing::info;
use tracing_subscriber::{prelude::*, EnvFilter};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputsFile {
    videos: Vec<VideoInput>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VideoInput {
    path: String,
    display_name: String,
}

#[tokio::main]
async fn main() {
    let video_inputs = fs::read_to_string("video_inputs.json")
        .expect("A file called video_inputs.json should exist in the current directory. [This is a hack for now].");
    let video_inputs: InputsFile =
        serde_json::from_str(&video_inputs).expect("video_inputs.json is invalid");
    if video_inputs.videos.is_empty() {
        panic!("video_inputs.json must contain some videos.")
    }

    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "phaneron=info")
    }

    let stdout_log = tracing_subscriber::fmt::layer().compact();
    let env_filter = EnvFilter::from_default_env();
    tracing_subscriber::registry()
        .with(stdout_log.with_filter(env_filter))
        .init();

    let phaneron_version = clap::crate_version!();
    info!("👋 Welcome to Phaneron version {phaneron_version}");

    let context = phaneron::create_compute_context().await;
    let state = PhaneronState::new(context.clone());

    info!("Loading plugins");
    let mut ffmpeg_producer_plugin = phaneron::FFmpegProducerPlugin::load();
    ffmpeg_producer_plugin.initialize().await;
    let mut traditional_mixer_emulator_plugin = phaneron::TraditionalMixerEmulatorPlugin::load();
    traditional_mixer_emulator_plugin.initialize().await;
    let mut turbo_consumer_plugin = phaneron::TurboConsumerPlugin::load();
    turbo_consumer_plugin.initialize().await;
    let mut webrtc_consumer_plugin = phaneron::WebRTCConsumerPlugin::load();
    webrtc_consumer_plugin.initialize().await;
    info!("Plugins loaded");

    let graph_id = phaneron::GraphId::new_from("graph1".to_string());

    let active_input_webrtc_consumer_id = phaneron::NodeId::new_from("webrtc_consumer".to_string());
    let active_input_webrtc_consumer_context = NodeContext::new(
        active_input_webrtc_consumer_id.clone(),
        context.clone(),
        state.clone(),
    );
    let mut active_input_webrtc_consumer = webrtc_consumer_plugin
        .create_node(active_input_webrtc_consumer_id.clone())
        .await;
    active_input_webrtc_consumer
        .initialize(active_input_webrtc_consumer_context.clone())
        .await;
    state
        .add_node(
            &graph_id,
            &active_input_webrtc_consumer_id,
            active_input_webrtc_consumer_context.clone(),
            Box::new(active_input_webrtc_consumer),
            Some("WebRTC Consumer (Active Input)".to_string()),
        )
        .await;

    let traditional_switcher_emulator_id = phaneron::NodeId::new_from("switcher".to_string());
    let traditional_switcher_emulator_context = NodeContext::new(
        traditional_switcher_emulator_id.clone(),
        context.clone(),
        state.clone(),
    );
    let mut traditional_switcher_emulator = traditional_mixer_emulator_plugin
        .create_node(traditional_switcher_emulator_id.clone())
        .await;
    traditional_switcher_emulator
        .initialize(
            traditional_switcher_emulator_context.clone(),
            TraditionalMixerEmulatorConfiguration {
                number_of_inputs: video_inputs.videos.len(),
            },
        )
        .await;
    state
        .add_node(
            &graph_id,
            &traditional_switcher_emulator_id,
            traditional_switcher_emulator_context.clone(),
            Box::new(traditional_switcher_emulator),
            Some("Switcher".to_string()),
        )
        .await;

    let mut audio_pipe: Option<AudioPipe> = None;
    for (i, input) in video_inputs.videos.iter().enumerate() {
        let ffmpeg_producer_id = phaneron::NodeId::default();
        let ffmpeg_producer_context =
            NodeContext::new(ffmpeg_producer_id.clone(), context.clone(), state.clone());
        let mut ffmpeg_producer = ffmpeg_producer_plugin
            .create_node(ffmpeg_producer_id.clone())
            .await;
        ffmpeg_producer
            .initialize(
                ffmpeg_producer_context.clone(),
                FFmpegProducerConfiguration {
                    file: input.path.to_string(),
                },
            )
            .await;
        state
            .add_node(
                &graph_id,
                &ffmpeg_producer_id,
                ffmpeg_producer_context.clone(),
                Box::new(ffmpeg_producer),
                Some(input.display_name.to_string()),
            )
            .await;

        let ffmpeg_producer_outputs = ffmpeg_producer_context.get_available_video_outputs().await;
        let ffmpeg_producer_pipe = ffmpeg_producer_context
            .get_video_pipe(ffmpeg_producer_outputs.first().unwrap())
            .await;
        traditional_switcher_emulator_context
            .connect_video_pipe(
                traditional_switcher_emulator_context
                    .get_available_video_inputs()
                    .await
                    .get(i)
                    .unwrap(),
                ffmpeg_producer_pipe,
            )
            .await
            .unwrap();

        // Connect first audio output
        if i == 0 {
            let ffmpeg_producer_audio_outputs =
                ffmpeg_producer_context.get_available_audio_outputs().await;
            if let Some(audio_out) = ffmpeg_producer_audio_outputs.first() {
                let ffmpeg_producer_audio_pipe =
                    ffmpeg_producer_context.get_audio_pipe(audio_out).await;
                audio_pipe = Some(ffmpeg_producer_audio_pipe);
            }
        }
    }

    // CONNECTIONS
    let emulator_outputs = traditional_switcher_emulator_context
        .get_available_video_outputs()
        .await;
    let emulator_pipe = traditional_switcher_emulator_context
        .get_video_pipe(emulator_outputs.first().unwrap())
        .await;

    active_input_webrtc_consumer_context
        .connect_video_pipe(
            &phaneron::VideoInputId::new_from("webrtc_consumer_video_input".to_string()),
            emulator_pipe,
        )
        .await
        .unwrap();

    if let Some(audio_pipe) = audio_pipe {
        active_input_webrtc_consumer_context
            .connect_audio_pipe(
                &phaneron::AudioInputId::new_from("webrtc_consumer_audio_input".to_string()),
                audio_pipe,
            )
            .await
            .unwrap();
    }

    let switcher_inputs = traditional_switcher_emulator_context
        .get_available_video_inputs()
        .await;
    let active_input = switcher_inputs.first().unwrap();
    info!("Setting active input to {active_input}");
    state
        .set_node_state(
            &graph_id,
            &traditional_switcher_emulator_id,
            serde_json::to_string(&TraditionalMixerEmulatorState {
                active_input: Some(active_input.to_string()),
                next_input: None,
                transition: None,
            })
            .unwrap(),
        )
        .await;

    phaneron::initialize_api(state.clone()).await;
}
