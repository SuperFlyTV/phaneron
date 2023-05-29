use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::{self, DirEntry},
    sync::{Arc, Mutex},
};

use abi_stable::{sabi_trait::TD_Opaque, std_types::RResult::ROk};
use phaneron_plugin::{
    traits::{NodeHandle_TO, Node_TO},
    ShaderParams, VideoInputId,
};
use serde::Deserialize;
use tracing::info;

use crate::compute::PhaneronComputeContext;

#[derive(Clone)]
struct PluginProvidedShader {
    name: String,
    shader: Arc<phaneron_plugin::types::ProcessShader>,
    args: Vec<ShaderArg>,
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
                if metadata.is_file()
                    && path.path().extension().and_then(OsStr::to_str).unwrap() == "cl"
                {
                    let shader = load_shader(context, path).unwrap();
                    println!("Loading {}", shader.0);
                    self.plugins.insert(shader.0, shader.1);
                    loaded_plugins += 1;
                }
            }
        }
        info!(
            "Loaded {} shader plugin{}",
            loaded_plugins,
            if loaded_plugins != 1 { "s" } else { "" }
        );
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShaderDescriptionFile {
    name: String,
    program_name: String,
    args: Vec<ShaderArg>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
enum ShaderArg {
    VideoInput {
        #[serde(rename = "displayName")]
        display_name: String,
    },
    VideoOutput {
        #[serde(rename = "displayName")]
        display_name: String,
    },
    F32 {
        key: String,
        #[serde(rename = "displayName")]
        display_name: String,
        #[serde(rename = "defaultVal")]
        default_val: f32,
    },
    U32 {
        key: String,
        #[serde(rename = "displayName")]
        display_name: String,
        #[serde(rename = "inclusiveMinimum")]
        inclusive_minimum: u32,
        #[serde(rename = "inclusiveMaximum")]
        inclusive_maximum: u32,
        #[serde(rename = "defaultVal")]
        default_val: u32,
    },
    Bool {
        key: String,
        #[serde(rename = "displayName")]
        display_name: String,
        #[serde(rename = "defaultVal")]
        default_val: bool,
    },
}

fn load_shader(
    context: &PhaneronComputeContext,
    path: DirEntry,
) -> anyhow::Result<(String, PluginProvidedShader)> {
    let mut shader_description_file = path.path();
    shader_description_file.set_extension("json");
    let shader = fs::read_to_string(path.path().to_str().unwrap()).unwrap();
    let shader_description = fs::read_to_string(shader_description_file).unwrap();
    let shader_description: ShaderDescriptionFile =
        serde_json::from_str(&shader_description).unwrap();
    let id = path.path();
    let id = id.file_stem().unwrap();
    let id = id.to_str().unwrap();

    let process_shader = context.create_process_shader(&shader, &shader_description.program_name);
    let shader = PluginProvidedShader {
        name: shader_description.name,
        shader: process_shader.into(),
        args: shader_description.args,
    };
    Ok((id.to_string(), shader))
}

impl phaneron_plugin::traits::PhaneronPlugin for ClShaderPlugin {
    fn get_available_node_types(
        &self,
    ) -> abi_stable::std_types::RVec<phaneron_plugin::traits::PluginNodeDescription> {
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
    > {
        let shader = self
            .plugins
            .get(&description.node_type.to_string())
            .unwrap();

        let handle = ShaderNodeHandle::new(description.node_id.into(), shader.clone());
        ROk(NodeHandle_TO::from_value(handle, TD_Opaque))
    }

    fn destroy_node(
        &self,
        node_id: abi_stable::std_types::RString,
    ) -> abi_stable::std_types::RResult<(), abi_stable::std_types::RString> {
        todo!()
    }
}

struct ShaderNodeHandle {
    id: String,
    shader: PluginProvidedShader,
}

impl ShaderNodeHandle {
    fn new(id: String, shader: PluginProvidedShader) -> Self {
        Self { id, shader }
    }
}

impl phaneron_plugin::traits::NodeHandle for ShaderNodeHandle {
    fn initialize(
        &self,
        context: phaneron_plugin::types::NodeContext,
        _configuration: abi_stable::std_types::ROption<abi_stable::std_types::RString>,
    ) -> phaneron_plugin::types::Node {
        let node = ShaderNode::new(
            self.id.clone(),
            context,
            self.shader.args.clone(),
            self.shader.shader.clone(),
        );

        Node_TO::from_value(node, TD_Opaque)
    }
}

enum ShaderRunArg {
    VideoInput {
        input_id: VideoInputId,
    },
    VideoOutput {
        output: phaneron_plugin::types::VideoOutput,
    },
    F32 {
        key: String,
        default_val: f32,
    },
    U32 {
        key: String,
        inclusive_minimum: u32,
        inclusive_maximum: u32,
        default_val: u32,
    },
    Bool {
        key: String,
        default_val: bool,
    },
}

struct ShaderNode {
    id: String,
    context: phaneron_plugin::types::NodeContext,
    run_args: Vec<ShaderRunArg>,
    shader: Arc<phaneron_plugin::types::ProcessShader>,
    state: Mutex<anymap::Map<dyn anymap::any::Any + Send + Sync>>,
}

impl ShaderNode {
    fn new(
        id: String,
        context: phaneron_plugin::types::NodeContext,
        args: Vec<ShaderArg>,
        shader: Arc<phaneron_plugin::types::ProcessShader>,
    ) -> Self {
        let mut run_args: Vec<ShaderRunArg> = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                ShaderArg::VideoInput { display_name: _ } => {
                    let input = context.add_video_input();
                    run_args.push(ShaderRunArg::VideoInput { input_id: input })
                }
                ShaderArg::VideoOutput { display_name: _ } => {
                    let output = context.add_video_output();
                    run_args.push(ShaderRunArg::VideoOutput { output })
                }
                ShaderArg::F32 {
                    key,
                    display_name: _,
                    default_val,
                } => run_args.push(ShaderRunArg::F32 { key, default_val }),
                ShaderArg::U32 {
                    key,
                    display_name: _,
                    default_val,
                    inclusive_minimum,
                    inclusive_maximum,
                } => run_args.push(ShaderRunArg::U32 {
                    key,
                    default_val,
                    inclusive_minimum,
                    inclusive_maximum,
                }),
                ShaderArg::Bool {
                    key,
                    display_name: _,
                    default_val,
                } => run_args.push(ShaderRunArg::Bool { key, default_val }),
            }
        }

        let mut state = anymap::Map::new();
        state.insert::<HashMap<String, f32>>(HashMap::new());
        state.insert::<HashMap<String, u32>>(HashMap::new());
        state.insert::<HashMap<String, bool>>(HashMap::new());

        let state = Mutex::new(state);

        Self {
            id,
            run_args,
            context,
            shader,
            state,
        }
    }
}

impl phaneron_plugin::traits::Node for ShaderNode {
    fn apply_state(&self, state: abi_stable::std_types::RString) -> bool {
        let json: serde_json::Value = serde_json::from_str(&state.to_string()).unwrap();

        for arg in self.run_args.iter() {
            match arg {
                ShaderRunArg::F32 { key, default_val } => {
                    // TODO: Validation
                    let val = &json[key];
                    let mut state_lock = self.state.lock().unwrap();
                    let f32_map = state_lock.get_mut::<HashMap<String, f32>>().unwrap();
                    if let serde_json::Value::Number(val) = val {
                        // TODO: Could panic
                        let mut val = val.as_f64().unwrap() as f32;
                        if val < 0.0 {
                            val = 0.0;
                        }

                        if val > 1.0 {
                            val = 1.0;
                        }
                        f32_map.insert(key.clone(), val);
                    } else {
                        f32_map.insert(key.clone(), *default_val);
                    }
                }
                ShaderRunArg::U32 {
                    key,
                    inclusive_minimum,
                    inclusive_maximum,
                    default_val,
                } => {
                    let val = &json[key];
                    let mut state_lock = self.state.lock().unwrap();
                    let f32_map = state_lock.get_mut::<HashMap<String, u32>>().unwrap();
                    if let serde_json::Value::Number(val) = val {
                        // TODO: Could panic
                        let mut val = val.as_u64().unwrap() as u32;
                        if val < *inclusive_minimum {
                            val = *inclusive_minimum;
                        }
                        if val > *inclusive_maximum {
                            val = *inclusive_maximum;
                        }
                        f32_map.insert(key.clone(), val);
                    } else {
                        f32_map.insert(key.clone(), *default_val);
                    }
                }
                ShaderRunArg::Bool { key, default_val } => {
                    let val = &json[key];
                    let mut state_lock = self.state.lock().unwrap();
                    let bool_map = state_lock.get_mut::<HashMap<String, bool>>().unwrap();
                    if let serde_json::Value::Bool(val) = val {
                        bool_map.insert(key.clone(), *val);
                    } else {
                        bool_map.insert(key.clone(), *default_val);
                    }
                }
                _ => {}
            }
        }

        true
    }

    fn process_frame(
        &self,
        frame_context: phaneron_plugin::types::ProcessFrameContext,
        video_frames: abi_stable::std_types::RHashMap<
            phaneron_plugin::VideoInputId,
            phaneron_plugin::VideoFrameWithId,
        >,
        _audio_frames: abi_stable::std_types::RHashMap<
            phaneron_plugin::AudioInputId,
            phaneron_plugin::AudioFrameWithId,
        >,
        black_frame: phaneron_plugin::VideoFrameWithId,
        _silence_frame: phaneron_plugin::AudioFrameWithId,
    ) {
        let mut params = ShaderParams::default();

        for arg in self.run_args.iter() {
            match arg {
                ShaderRunArg::VideoInput { input_id } => {
                    params.set_param_video_frame_input(
                        video_frames
                            .get(input_id)
                            .unwrap_or(&black_frame)
                            .clone()
                            .frame,
                    );
                }
                ShaderRunArg::VideoOutput { output: _ } => {
                    params.set_param_video_frame_output(1920, 1080) // TODO: Hard-coded dimensions
                }
                ShaderRunArg::F32 { key, default_val } => {
                    let state_lock = self.state.lock().unwrap();
                    let f32_map = state_lock.get::<HashMap<String, f32>>().unwrap();
                    params.set_param_f32_input(*f32_map.get(key).unwrap_or(default_val))
                }
                ShaderRunArg::U32 {
                    key,
                    inclusive_minimum: _,
                    inclusive_maximum: _,
                    default_val,
                } => {
                    let state_lock = self.state.lock().unwrap();
                    let u32_map = state_lock.get::<HashMap<String, u32>>().unwrap();
                    params.set_param_u32_input(*u32_map.get(key).unwrap_or(default_val))
                }
                ShaderRunArg::Bool { key, default_val } => {
                    let state_lock = self.state.lock().unwrap();
                    let bool_map = state_lock.get::<HashMap<String, bool>>().unwrap();
                    params.set_param_bool_input(*bool_map.get(key).unwrap_or(default_val))
                }
            }
        }

        let outputs = self.shader.run(params, &[1920, 1080]); // TODO: Hard-coded dimensions
        let frame_context = frame_context.submit().unwrap();

        for (index, output_frame) in outputs.into_iter().enumerate() {
            let video_output = self
                .run_args
                .iter()
                .filter(|arg| matches!(arg, ShaderRunArg::VideoOutput { output: _ }))
                .nth(index)
                .unwrap();
            // TODO: Messy
            match video_output {
                ShaderRunArg::VideoOutput { output } => {
                    output.push_frame(&frame_context, output_frame)
                }
                _ => unreachable!("Other shader args are filtered out"),
            }
        }
    }
}
