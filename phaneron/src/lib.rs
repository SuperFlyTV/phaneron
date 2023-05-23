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

pub use opencl3;

pub use crate::api::initialize_api;
pub use crate::compute::{audio_output::AudioPipe, create_compute_context};
pub use crate::graph::{GraphId, NodeId};
pub use crate::node_context::NodeRunContext;
pub use plugins::{
    cl_shader_plugin::ClShaderPlugin, DevPluginManifest, PluginLoadType, PluginManager,
};
pub use state::{
    create_phaneron_state, CreateConnection, CreateConnectionType, CreateNode, PhaneronState,
};

mod api;
mod channel;
mod colour;
mod compute;
mod format;
mod graph;
mod io;
mod load_save;
mod node_context;
mod plugins;
mod state;
