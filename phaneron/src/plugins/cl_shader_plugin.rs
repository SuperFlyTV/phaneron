use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::{self, DirEntry},
};

use anyhow::anyhow;
use serde::Deserialize;
use tracing::info;

use crate::compute::PhaneronComputeContext;

struct PluginProvidedShader {
    name: String,
    shader: phaneron_plugin::types::ProcessShader,
}

#[derive(Default)]
pub struct ClShaderPlugin {
    plugins: HashMap<String, PluginProvidedShader>,
}

impl ClShaderPlugin {
    pub fn load_from(&mut self, context: &PhaneronComputeContext, directory: std::path::PathBuf) {
        info!("Loading shader plugins");
        let mut loaded_plugins = 0;
        let paths = fs::read_dir(directory).unwrap();
        for path in paths.flatten() {
            if let Ok(metadata) = path.metadata() {
                if metadata.is_file() {
                    let shader = load_shader(context, path).unwrap();
                    self.plugins.insert(shader.0, shader.1);
                    loaded_plugins += 1;
                }
            }
        }
        info!("Loaded {} shader plugins", loaded_plugins);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShaderDescriptionFile {
    name: String,
    program_name: String,
}

fn load_shader(
    context: &PhaneronComputeContext,
    path: DirEntry,
) -> anyhow::Result<(String, PluginProvidedShader)> {
    if path.path().extension().and_then(OsStr::to_str).unwrap() == "cl" {
        let mut shader_description_file = path.path();
        shader_description_file.set_extension("json");
        let shader = fs::read_to_string(path.path().to_str().unwrap()).unwrap();
        let shader_description = fs::read_to_string(shader_description_file).unwrap();
        let shader_description: ShaderDescriptionFile =
            serde_json::from_str(&shader_description).unwrap();
        let id = path.file_name();
        let id = id.to_str().unwrap();

        let process_shader =
            context.create_process_shader(&shader, &shader_description.program_name);
        let shader = PluginProvidedShader {
            name: shader_description.name,
            shader: process_shader,
        };
        return Ok((id.to_string(), shader));
    }

    Err(anyhow!("Could not load shader"))
}

impl phaneron_plugin::traits::PhaneronPlugin for ClShaderPlugin {
    fn get_available_node_types(
        &self,
    ) -> abi_stable::std_types::RVec<phaneron_plugin::traits::PluginNodeDescription> where {
        let plugins: Vec<_> = self
            .plugins
            .iter()
            .map(|(k, v)| phaneron_plugin::traits::PluginNodeDescription {
                id: k.clone().into(),
                name: v.name.clone().into(),
            })
            .collect();
        plugins.into()
    }

    fn create_node(
        &self,
        description: phaneron_plugin::traits::CreateNodeDescription,
    ) -> abi_stable::std_types::RResult<
        phaneron_plugin::types::NodeHandle,
        abi_stable::std_types::RString,
    > where {
        todo!()
    }

    fn destroy_node(
        &self,
        node_id: abi_stable::std_types::RString,
    ) -> abi_stable::std_types::RResult<(), abi_stable::std_types::RString> where {
        todo!()
    }
}
