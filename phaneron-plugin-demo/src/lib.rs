use abi_stable::{
    export_root_module,
    prefix_type::PrefixTypeTrait,
    sabi_extern_fn,
    sabi_trait::TD_Opaque,
    std_types::{
        RResult::{self, RErr, ROk},
        RString, RVec,
    },
};
use phaneron_plugin::{
    traits::{CreateNodeDescription, PhaneronPlugin_TO},
    traits::{NodeHandle_TO, PluginNodeDescription},
    types::NodeHandle,
    types::PhaneronPlugin,
    PhaneronPluginContext, PhaneronPluginRootModule, PhaneronPluginRootModuleRef,
};

use self::{
    traditional_mixer_emulator::TraditionalMixerEmulatorHandle, turbo_consumer::TurboConsumerHandle,
};

mod dissolve;
mod traditional_mixer_emulator;
mod turbo_consumer;

pub use traditional_mixer_emulator::TraditionalMixerEmulatorState;

#[export_root_module]
fn instantiate_root_module() -> PhaneronPluginRootModuleRef {
    PhaneronPluginRootModule { load }.leak_into_prefix()
}

#[sabi_extern_fn]
pub fn load(context: PhaneronPluginContext) -> RResult<PhaneronPlugin, RString> {
    phaneron_plugin::get_logger(&context).init().unwrap();
    let plugin = DemoPlugin {};

    ROk(PhaneronPlugin_TO::from_value(plugin, TD_Opaque))
}

struct DemoPlugin {}
impl phaneron_plugin::traits::PhaneronPlugin for DemoPlugin {
    fn get_available_node_types(&self) -> RVec<PluginNodeDescription> {
        vec![
            PluginNodeDescription {
                id: "traditional_mixer_emulator".into(),
                name: "Traditional Mixer Emulator".into(),
            },
            PluginNodeDescription {
                id: "turbo_consumer".into(),
                name: "Turbo Consumer".into(),
            },
        ]
        .into()
    }

    fn create_node(&self, description: CreateNodeDescription) -> RResult<NodeHandle, RString> {
        match description.node_type.as_str() {
            "traditional_mixer_emulator" => {
                let handle = TraditionalMixerEmulatorHandle::new(description.node_id.to_string());

                ROk(NodeHandle_TO::from_value(handle, TD_Opaque))
            }
            "turbo_consumer" => {
                let handle = TurboConsumerHandle::new(description.node_id.to_string());

                ROk(NodeHandle_TO::from_value(handle, TD_Opaque))
            }
            _ => RErr(format!("Unknown node type: {}", description.node_type).into()),
        }
    }

    fn destroy_node(&self, node_id: RString) -> RResult<(), RString> {
        todo!()
    }
}
