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

use std::{
    collections::HashMap,
    fmt::Display,
    fs, io,
    path::{Path, PathBuf},
};

use abi_stable::{
    library::{lib_header_from_path, LibrarySuffix, RawLibrary},
    reexports::SelfOps,
    sabi_trait::TD_Opaque,
    std_types::ROption::{RNone, RSome},
};
use anyhow::anyhow;
use phaneron_plugin::{
    traits::{CreateNodeDescription, PluginNodeDescription},
    types::Node,
    types::NodeContext,
    types::NodeHandle,
    types::PhaneronPlugin,
    PhaneronLoggingContext, PhaneronLoggingContext_TO, PhaneronPluginContext,
    PhaneronPluginRootModuleRef,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub(super) mod cl_shader_plugin;

#[derive(Debug, Deserialize)]
pub struct DevPluginManifest {
    plugins: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct PhaneronPluginsState {
    plugins_and_node_types: HashMap<PluginId, Vec<String>>,
    node_descriptions: HashMap<String, PhaneronPluginNode>,
}

#[derive(Debug, Clone)]
pub struct PhaneronPluginNode {
    id: String,
    name: String,
}

#[derive(Default)]
pub struct PluginManager {
    plugins: HashMap<PluginId, PhaneronPlugin>,
    nodes_provided_by_plugins: HashMap<String, PluginId>,
    node_descriptions: HashMap<String, PluginNodeDescription>,
    subscribers_to_state: Mutex<Vec<tokio::sync::broadcast::Sender<PhaneronPluginsState>>>,
}

pub enum PluginLoadType {
    Development(DevPluginManifest),
    Production { plugins_directory: String },
}

impl PluginManager {
    pub fn load_from(&mut self, load_type: PluginLoadType) -> anyhow::Result<usize> {
        let (plugins_to_load, plugins_directory) = match load_type {
            PluginLoadType::Development(manifest) => (manifest.plugins, None),
            PluginLoadType::Production { plugins_directory } => {
                let mut plugins = vec![];
                let paths = fs::read_dir(plugins_directory.clone()).unwrap();
                for path in paths.flatten() {
                    if let Ok(metadata) = path.metadata() {
                        if metadata.is_file() {
                            if let Some(name) = path.file_name().to_str() {
                                plugins.push(name.to_string())
                            }
                        }
                    }
                }
                (plugins, Some(plugins_directory))
            }
        };

        let mut loaded_plugins = 0;
        for plugin in plugins_to_load {
            self.load_plugin(&plugins_directory, &plugin)?;
            loaded_plugins += 1;
        }

        Ok(loaded_plugins)
    }

    pub fn add_plugin(&mut self, plugin: PhaneronPlugin) -> anyhow::Result<()> {
        let nodes = plugin.get_available_node_types();

        let plugin_id = PluginId::default();
        for node_type in nodes {
            self.nodes_provided_by_plugins
                .insert(node_type.id.to_string(), plugin_id.clone());
            self.node_descriptions
                .insert(node_type.id.to_string(), node_type);
        }
        self.plugins.insert(plugin_id, plugin);

        Ok(())
    }

    fn load_plugin(
        &mut self,
        plugins_dir: &Option<String>,
        plugin_name: &str,
    ) -> anyhow::Result<()> {
        let library_path: PathBuf = match compute_plugin_path(plugins_dir, plugin_name) {
            Ok(x) => x,
            Err(e) => return Err(anyhow!(e)),
        };

        let res = (|| {
            let header = lib_header_from_path(&library_path)?;
            header.init_root_module::<PhaneronPluginRootModuleRef>()
        })();

        let root_module = match res {
            Ok(x) => x,
            Err(e) => return Err(anyhow!(e)),
        };

        let logger = PluginLogger {
            plugin_name: plugin_name.to_string(),
        };
        let logger = PhaneronLoggingContext_TO::from_value(logger, TD_Opaque);
        let plugin_context = PhaneronPluginContext::new(logger);
        let plugin = root_module.load()(plugin_context)
            .map_err(|err| anyhow!(err.to_string()))
            .into_result()?;
        let nodes = plugin.get_available_node_types();

        let plugin_id = PluginId::default();
        for node_type in nodes {
            self.nodes_provided_by_plugins
                .insert(node_type.id.to_string(), plugin_id.clone());
            self.node_descriptions
                .insert(node_type.id.to_string(), node_type);
        }
        self.plugins.insert(plugin_id, plugin);

        Ok(())
    }

    pub fn create_node_handle(
        &self,
        node_id: String,
        node_type: String,
    ) -> Result<NodeHandle, String> {
        let plugin_id = self.nodes_provided_by_plugins.get(&node_type).unwrap();
        let plugin = self.plugins.get(plugin_id).unwrap();
        plugin
            .create_node(CreateNodeDescription {
                node_type: node_type.into(),
                node_id: node_id.into(),
            })
            .map_err(|err| err.into())
            .into()
    }

    pub fn initialize_node(
        &self,
        context: NodeContext,
        node_handle: NodeHandle,
        configuration: Option<String>,
    ) -> Option<Node> {
        let configuration = match configuration {
            Some(config) => RSome(config.into()),
            None => RNone,
        };
        Some(node_handle.initialize(context, configuration))
    }

    pub async fn subscribe_to_plugins(
        &self,
    ) -> tokio::sync::broadcast::Receiver<PhaneronPluginsState> {
        let (sender, receiver) = tokio::sync::broadcast::channel(1); // Only the latest value is relevant

        {
            let state = self.get_state().await;
            sender.send(state).unwrap();
        }
        self.subscribers_to_state.lock().await.push(sender);

        receiver
    }

    async fn get_state(&self) -> PhaneronPluginsState {
        let mut plugins_and_node_types: HashMap<PluginId, Vec<String>> = HashMap::new();
        for (node_id, plugin_id) in self.nodes_provided_by_plugins.iter() {
            let entry = plugins_and_node_types.entry(plugin_id.clone()).or_default();
            entry.push(node_id.clone());
        }

        let node_descriptions = self
            .node_descriptions
            .iter()
            .map(|(id, desc)| {
                (
                    id.clone(),
                    PhaneronPluginNode {
                        id: desc.id.clone().into(),
                        name: desc.name.clone().into(),
                    },
                )
            })
            .collect();

        PhaneronPluginsState {
            plugins_and_node_types,
            node_descriptions,
        }
    }
}

fn compute_plugin_path(plugins_dir: &Option<String>, base_name: &str) -> io::Result<PathBuf> {
    if let Some(plugins_dir) = plugins_dir {
        let plugins_dir = plugins_dir.as_ref_::<Path>().into_::<PathBuf>();
        let plugins_path =
            RawLibrary::path_in_directory(&plugins_dir, base_name, LibrarySuffix::NoSuffix);

        return Ok(plugins_path);
    }

    let debug_dir = "target/debug/".as_ref_::<Path>().into_::<PathBuf>();
    let release_dir = "target/release/".as_ref_::<Path>().into_::<PathBuf>();

    let debug_path = RawLibrary::path_in_directory(&debug_dir, base_name, LibrarySuffix::NoSuffix);
    let release_path =
        RawLibrary::path_in_directory(&release_dir, base_name, LibrarySuffix::NoSuffix);

    match (debug_path.exists(), release_path.exists()) {
        (false, false) => debug_path,
        (true, false) => debug_path,
        (false, true) => release_path,
        (true, true) => {
            if debug_path.metadata()?.modified()? < release_path.metadata()?.modified()? {
                release_path
            } else {
                debug_path
            }
        }
    }
    .piped(Ok)
}

#[derive(Debug, Clone)]
struct PluginLogger {
    plugin_name: String,
}
impl PhaneronLoggingContext for PluginLogger {
    fn log(&self, level: phaneron_plugin::LogLevel, message: abi_stable::std_types::RString) {
        match level {
            phaneron_plugin::LogLevel::Error => {
                tracing::error!("PLUGIN {}: {}", self.plugin_name, message.to_string())
            }
            phaneron_plugin::LogLevel::Warn => {
                tracing::warn!("PLUGIN {}: {}", self.plugin_name, message.to_string())
            }
            phaneron_plugin::LogLevel::Info => {
                tracing::info!("PLUGIN {}: {}", self.plugin_name, message.to_string())
            }
            phaneron_plugin::LogLevel::Debug => {
                tracing::debug!("PLUGIN {}: {}", self.plugin_name, message.to_string())
            }
            phaneron_plugin::LogLevel::Trace => {
                tracing::trace!("PLUGIN {}: {}", self.plugin_name, message.to_string())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginId(String);
impl PluginId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for PluginId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for PluginId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
