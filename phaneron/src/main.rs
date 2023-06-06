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

use std::{fs, path::Path};

use abi_stable::sabi_trait::TD_Opaque;
use phaneron::{
    create_phaneron_state, ClShaderPlugin, CreateConnection, CreateConnectionType, CreateNode,
    DevPluginManifest, NodeId, PluginLoadType, PluginManager,
};
use phaneron_plugin::traits::PhaneronPlugin_TO;
use serde::{Deserialize, Serialize};
use tracing::info;
use tracing_subscriber::{prelude::*, EnvFilter};

// TODO: Remove
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
pub struct FFmpegProducerState {
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "transition")]
pub enum TraditionalMixerEmulatorTransition {
    Mix { position: f32 },
}

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
    #[cfg(debug_assertions)]
    dotenv::dotenv().ok();

    let video_inputs = fs::read_to_string("video_inputs.json")
        .expect("A file called video_inputs.json should exist in the current directory. [This is a hack for now].");
    let video_inputs: InputsFile =
        serde_json::from_str(&video_inputs).expect("video_inputs.json is invalid");
    if video_inputs.videos.is_empty() {
        panic!("video_inputs.json must contain some videos.")
    }

    #[cfg(debug_assertions)]
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1")
    }

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "phaneron=info")
    }

    let develop_plugins = std::env::var("DEVELOP_PLUGINS");
    let plugin_load_type = match develop_plugins {
        Ok(_) => {
            let plugins_cfg_file =
                std::env::var("PLUGINS_CFG_FILE").unwrap_or("plugins.toml".to_string());
            let plugins = fs::read_to_string(plugins_cfg_file).unwrap();
            let plugins: DevPluginManifest = toml::from_str(&plugins).unwrap();
            PluginLoadType::Development(plugins)
        }
        Err(_) => {
            let plugins_directory =
                std::env::var("PLUGINS_DIRECTORY").unwrap_or("plugins".to_string());
            PluginLoadType::Production { plugins_directory }
        }
    };

    let shader_plugins_directory =
        std::env::var("SHADER_PLUGINS_DIR").unwrap_or_else(|_| match &plugin_load_type {
            PluginLoadType::Development(_) => "phaneron-plugin-shaders".to_string(),
            PluginLoadType::Production { plugins_directory } => plugins_directory.clone(),
        });

    let stdout_log = tracing_subscriber::fmt::layer().compact();
    let env_filter = EnvFilter::from_default_env();
    tracing_subscriber::registry()
        .with(stdout_log.with_filter(env_filter))
        .init();

    let phaneron_version = clap::crate_version!();
    info!("👋 Welcome to Phaneron version {phaneron_version}");
    info!(
        "Phaneron Copyright (C) 2023 SuperFlyTV AB. This program comes with ABSOLUTELY NO WARRANTY. This is free software, and you are welcome to redistribute it under certain conditions; refer to the LICENSE for details."
    );

    let context = phaneron::create_compute_context().await;
    let state = create_phaneron_state(context.clone());

    info!("Loading plugins");
    let mut plugin_manager = PluginManager::default();
    let loaded_plugins = plugin_manager.load_from(plugin_load_type).unwrap();
    info!(
        "Loaded {} plugin{}",
        loaded_plugins,
        if loaded_plugins != 1 { "s" } else { "" }
    );

    let mut shader_plugin = ClShaderPlugin::default();
    shader_plugin.load_from(&context, Path::new(&shader_plugins_directory).into());
    plugin_manager
        .add_plugin(PhaneronPlugin_TO::from_value(shader_plugin, TD_Opaque))
        .unwrap();

    let graph_id = phaneron::GraphId::new_from("graph1".to_string());
    let mut create_nodes = vec![
        CreateNode {
            node_id: "active_input_webrtc_consumer".to_string(),
            node_type: "webrtc_consumer".to_string(),
            node_name: None,
            state: None,
            configuration: None,
        },
        CreateNode {
            node_id: "switcher".to_string(),
            node_type: "traditional_mixer_emulator".to_string(),
            node_name: None,
            state: Some(
                serde_json::to_string(&TraditionalMixerEmulatorState {
                    active_input: None,
                    next_input: None,
                    transition: None,
                })
                .unwrap(),
            ),
            configuration: Some(
                serde_json::to_string(&TraditionalMixerEmulatorConfiguration {
                    number_of_inputs: video_inputs.videos.len(),
                })
                .unwrap(),
            ),
        },
        CreateNode {
            node_id: "flipper".to_string(),
            node_type: "flip".to_string(),
            node_name: Some("flip".to_string()),
            state: None,
            configuration: None,
        },
    ];
    let mut connections = vec![
        CreateConnection {
            connection_type: CreateConnectionType::Video,
            from_node_id: "switcher".to_string(),
            from_output_index: 0,
            to_node_id: "flipper".to_string(),
            to_input_index: 0,
        },
        CreateConnection {
            connection_type: CreateConnectionType::Video,
            from_node_id: "flipper".to_string(),
            from_output_index: 0,
            to_node_id: "active_input_webrtc_consumer".to_string(),
            to_input_index: 0,
        },
    ];
    for (index, input) in video_inputs.videos.iter().enumerate() {
        let ffmpeg_producer_id = phaneron::NodeId::default();
        create_nodes.push(CreateNode {
            node_id: ffmpeg_producer_id.to_string(),
            node_type: "ffmpeg_producer".to_string(),
            node_name: Some(input.display_name.clone()),
            state: Some(
                serde_json::to_string(&FFmpegProducerState {
                    file: input.path.to_string(),
                })
                .unwrap(),
            ),
            configuration: None,
        });
        connections.push(CreateConnection {
            connection_type: CreateConnectionType::Video,
            from_node_id: ffmpeg_producer_id.to_string(),
            from_output_index: 0,
            to_node_id: "switcher".to_string(),
            to_input_index: index,
        });

        // Connect first audio output
        if index == 0 {
            connections.push(CreateConnection {
                connection_type: CreateConnectionType::Audio,
                from_node_id: ffmpeg_producer_id.to_string(),
                from_output_index: 0,
                to_node_id: "active_input_webrtc_consumer".to_string(),
                to_input_index: 0,
            });
        }
    }

    state
        .create_graph(&plugin_manager, &graph_id, create_nodes, connections)
        .await
        .unwrap();

    let available_inputs = state
        .get_available_video_inputs(&graph_id, &NodeId::new_from("switcher".to_string()))
        .await;

    let switcher_state = state
        .get_node_state(&graph_id, &NodeId::new_from("switcher".to_string()))
        .await
        .unwrap();
    let mut switcher_state: TraditionalMixerEmulatorState =
        serde_json::from_str(&switcher_state).unwrap();
    switcher_state.active_input = Some(available_inputs.first().unwrap().to_string());
    state
        .set_node_state(
            &graph_id,
            &NodeId::new_from("switcher".to_string()),
            serde_json::to_string(&switcher_state).unwrap(),
        )
        .await;

    phaneron::initialize_api(state.clone(), &plugin_manager).await;
}
