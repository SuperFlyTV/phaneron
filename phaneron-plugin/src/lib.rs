//! This module defines the interfaces to be implemented for a Phaneron plugin to be successfully loaded.
//!
//! To create a plugin you must have a function annotated with the `#[export_root_module]` from the `abi_stable` crate.
//! This function should return a `PhaneronPluginRootModule` which acts as the handle which can be used to initialize
//! your plugin. This first function should not perform any additional work such as loading assets etc. as your plugin will be given
//! an opportunity to initialize itself later.
//!
//! You should then have a `load` function that is annotated using the `#[sabi_external_fn]` macro from the `abi_stable` crate.
//! This function is the initializer for your plugin and is where you can load assets that are globally required and pre-allocate
//! large amounts of memory if required. This function is allowed to fail and will only be called once. If it fails then the plugin
//! will not be loaded and Phaneron will not attempt to load the plugin again. If failing, please return some useful error message.
//!
//! ```
//! # use abi_stable::{
//! #     export_root_module,
//! #     prefix_type::PrefixTypeTrait,
//! #     sabi_extern_fn,
//! #     sabi_trait::TD_Opaque,
//! #     std_types::{
//! #         RResult::{self, RErr, ROk},
//! #         RString, RVec,
//! #     },
//! # };
//! # use log::LevelFilter;
//! # use phaneron_plugin::{
//! #     traits::NodeHandle_TO, traits::PhaneronPlugin_TO, types::NodeHandle, types::PhaneronPlugin,
//! #     CreateNodeDescription, PhaneronPluginContext, PhaneronPluginRootModule,
//! #     PhaneronPluginRootModuleRef, PluginNodeDescription,
//! # };
//! #[export_root_module]
//! fn instantiate_root_module() -> PhaneronPluginRootModuleRef {
//!     PhaneronPluginRootModule { load }.leak_into_prefix()
//! }
//!
//! #[sabi_extern_fn]
//! pub fn load(context: PhaneronPluginContext) -> RResult<PhaneronPlugin, RString> {
//!     log::set_logger(phaneron_plugin::get_logger(&context)).unwrap();
//!     log::set_max_level(LevelFilter::Trace);
//!     let plugin = DemoPlugin {};
//!
//!     ROk(PhaneronPlugin_TO::from_value(plugin, TD_Opaque))
//! }
//!
//! # struct DemoPlugin {}
//! # impl phaneron_plugin::traits::PhaneronPlugin for DemoPlugin {
//! #     fn get_available_node_types(&self) -> RVec<PluginNodeDescription> {
//! #         todo!()
//! #     }
//! #
//! #     fn create_node(&self, description: CreateNodeDescription) -> RResult<NodeHandle, RString> {
//! #         todo!()
//! #     }
//! #
//! #     fn destroy_node(&self, node_id: RString) -> RResult<(), RString> {
//! #         todo!()
//! #     }
//! # }
//! ```
//!
//! The returned `plugin` is of type `DemoPlugin` in this instance, which implements the [`PhaneronPlugin`](traits::PhaneronPlugin) trait. This object will be
//! used to create nodes from your plugin and manage its lifecycle. Refer to the documentation of [`PhaneronPlugin`](traits::PhaneronPlugin).

use std::fmt::Debug;

use abi_stable::{
    declare_root_module_statics,
    library::RootModule,
    package_version_strings, sabi_trait,
    sabi_types::VersionStrings,
    std_types::{RBox, RResult, RString, RVec},
    StableAbi,
};
use log::{LevelFilter, SetLoggerError};
use once_cell::sync::OnceCell;
use types::PhaneronPlugin;

pub use crate::{
    audio::{AudioChannelLayout, AudioFormat},
    colour::*,
    graph::{AudioInputId, AudioOutputId, VideoInputId, VideoOutputId},
    video::{InterlaceMode, VideoFormat},
};

mod audio;
mod colour;
mod graph;
mod video;

pub mod traits;
pub mod types;

/// A single logging instance is available to each individual plugin, so a plugin
/// may request its context multiple times and get a reference to the same value.
static LOGGER: OnceCell<PluginLogger> = OnceCell::new();

/// Context passed to plugins, currently only serves as a way to provide a logging contet.
#[repr(C)]
#[derive(StableAbi)]
pub struct PhaneronPluginContext {
    logging_context: PhaneronLoggingContext_TO<'static, RBox<()>>,
}

impl PhaneronPluginContext {
    pub fn new(logging_context: PhaneronLoggingContext_TO<'static, RBox<()>>) -> Self {
        PhaneronPluginContext { logging_context }
    }
}

impl Debug for PhaneronPluginContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhaneronPluginContext").finish()
    }
}

/// This should not be used by plugins, it is consumed by Phaneron to carry log messages
/// across the FFI boundary.
#[repr(usize)]
#[derive(StableAbi)]
pub enum LogLevel {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<log::Level> for LogLevel {
    fn from(value: log::Level) -> Self {
        match value {
            log::Level::Error => LogLevel::Error,
            log::Level::Warn => LogLevel::Warn,
            log::Level::Info => LogLevel::Info,
            log::Level::Debug => LogLevel::Debug,
            log::Level::Trace => LogLevel::Trace,
        }
    }
}

/// This trait is used to allow the logger to be used across the FFI boundary. It should not be consumed by plugins.
#[sabi_trait]
pub trait PhaneronLoggingContext: Send + Sync + Clone {
    fn log(&self, level: LogLevel, message: RString);
}

/// Describes the entrypoint for a plugin.
#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = PhaneronPluginRootModuleRef)))]
#[sabi(missing_field(panic))]
pub struct PhaneronPluginRootModule {
    #[sabi(last_prefix_field)]
    pub load:
        extern "C" fn(load_context: PhaneronPluginContext) -> RResult<PhaneronPlugin, RString>,
}

impl RootModule for PhaneronPluginRootModuleRef {
    declare_root_module_statics! {PhaneronPluginRootModuleRef}
    const BASE_NAME: &'static str = "phaneron-plugin";
    const NAME: &'static str = "phaneron-plugin";
    const VERSION_STRINGS: VersionStrings = package_version_strings!();
}

/// A video frame along with its associated output Id.
#[repr(C)]
#[derive(Clone, StableAbi)]
pub struct VideoFrameWithId {
    pub output_id: VideoOutputId,
    pub frame: types::VideoFrame,
}

impl VideoFrameWithId {
    pub fn new(output_id: VideoOutputId, frame: types::VideoFrame) -> Self {
        Self { output_id, frame }
    }
}

/// An audio frame along with its associated output Id.
#[repr(C)]
#[derive(Clone, StableAbi)]
pub struct AudioFrameWithId {
    pub output_id: AudioOutputId,
    pub frame: types::AudioFrame,
}

impl AudioFrameWithId {
    pub fn new(output_id: AudioOutputId, frame: types::AudioFrame) -> Self {
        Self { output_id, frame }
    }
}

/// Parameters to be passed to a process shader.
/// Each call to a `set_param_` function will push a parameter of that
/// type to the arguments list, so calls should be made in the order in which
/// parameters are required to be sent to the shader.
#[repr(C)]
#[derive(Default, StableAbi)]
pub struct ShaderParams {
    params: RVec<ShaderParam>,
}
impl ShaderParams {
    pub fn set_param_video_frame_input(&mut self, video_frame: types::VideoFrame) {
        self.params.push(ShaderParam::VideoFrameInput(video_frame));
    }

    pub fn set_param_u32_input(&mut self, val: u32) {
        self.params.push(ShaderParam::U32Input(val));
    }

    pub fn set_param_f32_input(&mut self, val: f32) {
        self.params.push(ShaderParam::F32Input(val));
    }

    pub fn set_param_bool_input(&mut self, val: bool) {
        self.params.push(ShaderParam::Bool(val));
    }

    /// Sets a video frame as an output of a shader.
    pub fn set_param_video_frame_output(&mut self, width: usize, height: usize) {
        self.params
            .push(ShaderParam::VideoFrameOutput { width, height });
    }

    pub fn get_params(&self) -> &RVec<ShaderParam> {
        &self.params
    }
}

/// Available shader parameter types, these do not need to be directly consumed by plugins.
#[repr(C)]
#[derive(StableAbi)]
pub enum ShaderParam {
    VideoFrameInput(types::VideoFrame),
    U32Input(u32),
    F32Input(f32),
    Bool(bool),
    VideoFrameOutput { width: usize, height: usize },
}

/// Provides logging to a plugin.
/// It can be set as the default logger for the `log` trait by calling `init`.
pub struct PluginLogger {
    context: PhaneronLoggingContext_TO<'static, RBox<()>>,
}

impl PluginLogger {
    /// Sets this logger as the global logger for `log`.
    /// Returns an error if a logger has already been set as the
    /// global logger.
    pub fn init(&'static self) -> Result<(), SetLoggerError> {
        log::set_logger(self)?;
        log::set_max_level(LevelFilter::Trace);
        Ok(())
    }
}

impl Debug for PluginLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginLogger").finish()
    }
}

impl log::Log for PluginLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            self.context
                .log(record.level().into(), record.args().to_string().into())
        }
    }

    fn flush(&self) {}
}

/// Returns the logger for a plugin which can then be used to pass log messages to Phaneron.
pub fn get_logger(context: &PhaneronPluginContext) -> &'static PluginLogger {
    let logger = PluginLogger {
        context: context.logging_context.clone(),
    };
    LOGGER.set(logger).unwrap();
    LOGGER.get().unwrap()
}
