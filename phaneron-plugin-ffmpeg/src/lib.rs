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

use self::ffmpeg_producer::FFmpegProducerHandle;

use abi_stable::{
    export_root_module,
    prefix_type::PrefixTypeTrait,
    sabi_extern_fn,
    sabi_trait::TD_Opaque,
    std_types::{
        RResult::{self, ROk},
        RString, RVec,
    },
};
use phaneron_plugin::{
    traits::NodeHandle_TO,
    traits::{CreateNodeDescription, PhaneronPlugin_TO, PluginNodeDescription},
    types::NodeHandle,
    types::PhaneronPlugin,
    PhaneronPluginContext, PhaneronPluginRootModule, PhaneronPluginRootModuleRef,
};

mod ffmpeg_producer;
pub use ffmpeg_producer::FFmpegProducerState;

#[export_root_module]
fn instantiate_root_module() -> PhaneronPluginRootModuleRef {
    PhaneronPluginRootModule { load }.leak_into_prefix()
}

#[sabi_extern_fn]
pub fn load(context: PhaneronPluginContext) -> RResult<PhaneronPlugin, RString> {
    phaneron_plugin::get_logger(&context).init().unwrap();
    let plugin = FFmpegPlugin {};

    ROk(PhaneronPlugin_TO::from_value(plugin, TD_Opaque))
}

struct FFmpegPlugin {}
impl phaneron_plugin::traits::PhaneronPlugin for FFmpegPlugin {
    fn get_available_node_types(&self) -> RVec<PluginNodeDescription> {
        vec![PluginNodeDescription {
            id: "ffmpeg_producer".into(),
            name: "FFmpeg producer".into(),
        }]
        .into()
    }

    fn create_node(&self, description: CreateNodeDescription) -> RResult<NodeHandle, RString> {
        let handle = FFmpegProducerHandle::new(description.node_id.into());

        ROk(NodeHandle_TO::from_value(handle, TD_Opaque))
    }

    fn destroy_node(&self, node_id: RString) -> RResult<(), RString> {
        todo!()
    }
}
