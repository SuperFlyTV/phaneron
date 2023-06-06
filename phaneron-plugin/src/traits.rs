pub use crate::{
    audio::{AudioChannelLayout, AudioFormat},
    colour::*,
    graph::{AudioInputId, AudioOutputId, VideoInputId, VideoOutputId},
    video::{InterlaceMode, VideoFormat},
    AudioFrameWithId, VideoFrameWithId,
};
use abi_stable::{
    sabi_trait,
    std_types::{RHashMap, ROption, RResult, RSlice, RStr, RString, RVec},
    StableAbi,
};

#[sabi_trait]
pub trait PhaneronPlugin: Send + Sync {
    fn get_available_node_types(&self) -> RVec<PluginNodeDescription>;
    fn create_node(
        &self,
        description: CreateNodeDescription,
    ) -> RResult<crate::types::NodeHandle, RString>;
    fn destroy_node(&self, node_id: RString) -> RResult<(), RString>;
}

/// Provides a description of an available node type provided by a plugin.
#[repr(C)]
#[derive(StableAbi)]
pub struct PluginNodeDescription {
    /// The Id that should be passed to `create_node` in order to create a node of this type
    pub id: RString,
    /// Human-readable name of the node
    pub name: RString,
}

/// Passed to a plugin in order to request the creation of a node.
#[repr(C)]
#[derive(StableAbi)]
pub struct CreateNodeDescription {
    /// Id the node that will be used to refer to the node for the entirety of its lifespan.
    pub node_id: RString,
    /// Type of node that is being requested for creation (provided by the plugin).
    pub node_type: RString,
}

/// A handle to a node that can be initialized later.
/// Serves as a reservation of the node and associated resources.
#[sabi_trait]
pub trait NodeHandle: Send + Sync {
    fn initialize(
        &self,
        context: crate::types::NodeContext,
        configuration: ROption<RString>,
    ) -> crate::types::Node;
}

/// An initialized node.
#[sabi_trait]
pub trait Node: Send + Sync {
    /// Apply the given state, return true if the state has been successfully applied
    fn apply_state(&self, state: RString) -> bool;
    /// Called when the node should produce a frame.
    fn process_frame(&self, frame_context: crate::types::ProcessFrameContext);
}

/// Context provided to nodes when they are initialized.
#[sabi_trait]
pub trait NodeContext: Send + Sync {
    /// Add an audio input to the node.
    fn add_audio_input(&self) -> AudioInputId;
    /// Add a video input to the node.
    fn add_video_input(&self) -> VideoInputId;
    /// Add an audio output to the node.
    fn add_audio_output(&self) -> crate::types::AudioOutput;
    /// Add a video output to the node.
    fn add_video_output(&self) -> crate::types::VideoOutput;
    /// Create a [`ToRGBA`] that can be used to load video frames onto the GPU.
    fn create_to_rgba(
        &self,
        video_format: &VideoFormat,
        colour_space: &ColourSpec,
        width: usize,
        height: usize,
    ) -> crate::types::ToRGBA;
    /// Create a ['FromRGBA`] that can be used to consume video frames from the GPU.
    fn create_from_rgba(
        &self,
        video_format: &VideoFormat,
        colour_space: &ColourSpec,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> crate::types::FromRGBA;
    /// Create a [`ToAudioF32`] that can be used to load audio in 32 bit floating-point format.
    fn create_to_audio_f32(
        &self,
        audio_format: AudioFormat,
        channel_layout: AudioChannelLayout,
    ) -> crate::types::ToAudioF32;
    /// Create a [`FromAudioF32`] that can be used to consume 32 bit floating-point audio in other formats.
    fn create_from_audio_f32(
        &self,
        audio_format: AudioFormat,
        channel_layout: AudioChannelLayout,
    ) -> crate::types::FromAudioF32;
    /// Create a shader from a source string.
    /// * `kernel` - Shader code.
    /// * `program_name` - Name of the kernel function.
    fn create_process_shader(
        &self,
        kernel: RStr<'_>,
        program_name: RStr<'_>,
    ) -> crate::types::ProcessShader;
}

/// Provides proof that frame processing operations can be performed.
/// Must be submitted to acquire a [`FrameContext`] that can be used for copy
/// operations. This proves that all of the processing operations for a node
/// have been submitted before the frame attempts to consume a frame.
#[sabi_trait]
pub trait ProcessFrameContext {
    /// Returns an error if called twice
    fn submit(&self) -> RResult<crate::types::FrameContext, RString>;
    fn get_video_input(&self, id: &VideoInputId) -> ROption<&crate::VideoFrameWithId>;
    fn get_audio_input(&self, id: &AudioInputId) -> ROption<&crate::AudioFrameWithId>;
    fn get_black_frame(&self) -> &crate::VideoFrameWithId;
    fn get_silence_frame(&self) -> &crate::AudioFrameWithId;
}

/// Provides proof that frame copy operations can be performed.
#[sabi_trait]
pub trait FrameContext {}

/// Once a process shader has been created, this trait allows a node
/// to interact with the shader.
#[sabi_trait]
pub trait ProcessShader: Send + Sync {
    fn run(
        &self,
        params: crate::ShaderParams,
        global_work_size: &[usize; 2],
    ) -> RVec<crate::types::VideoFrame>;
}

/// Provides a handle to a video frame on the GPU.
#[sabi_trait]
pub trait VideoFrame: Send + Sync {
    fn buffer_index(&self) -> usize;
    fn width(&self) -> usize;
    fn height(&self) -> usize;
}

/// Provides a handle to an audio frame (and the data).
#[sabi_trait]
pub trait AudioFrame: Send + Sync {
    fn buffers(&self) -> &RVec<RVec<f32>>;
}

/// A video output from a node, this is where the video frames a node creates should be sent to
/// be forwarded to other nodes.
#[sabi_trait]
pub trait VideoOutput: Send + Sync {
    fn push_frame(&self, context: &crate::types::FrameContext, frame: crate::types::VideoFrame);
}

/// An audio output from a node, this is where the audio frames a node creates shoud be sent to
/// be forwarded to other nodes.
#[sabi_trait]
pub trait AudioOutput: Send + Sync {
    fn push_frame(&self, context: &crate::types::FrameContext, frame: crate::types::AudioFrame);
}

/// Provides functions for loading video frames onto the GPU.
#[sabi_trait]
pub trait ToRGBA: Send + Sync {
    /// Used internally by Phaneron to get the size of a frame before loading.
    fn get_num_bytes(&self) -> RVec<usize>;
    /// Used internally by Phaneron to get the size of a frame after it is loaded onto the GPU.
    fn get_num_bytes_rgba(&self) -> usize;
    /// Used internally by Phaneron during the loading process.
    fn get_total_bytes(&self) -> usize;
    /// Loads a frame onto the GPU. For YUV formats, each of the Y,U, and V planes should be
    /// provided in its own slice, thus a slice of slices is taken as an argument.
    /// This does not perform the format conversion, only the copy.
    /// Both this function and `process_frame` may be called in a background task/thread outside of the
    /// `process_frame` function. A node should ensure that it only copies a "reasonable" amount of frames
    /// onto the GPU at any point in time.
    fn load_frame(&self, inputs: &RSlice<RSlice<u8>>) -> crate::types::LoadedVideoFrame;
    /// After loading a frame it must be processed into the common video format by calling this function.
    fn process_frame(&self, sources: crate::types::LoadedVideoFrame) -> crate::types::VideoFrame;
}

/// A handle to a video frame that has been loaded onto the GPU but not yet converted.
#[sabi_trait]
pub trait LoadedVideoFrame {}

/// Provides functions for consuming video frames from the GPU.
#[sabi_trait]
pub trait FromRGBA: Send + Sync {
    /// Can be used to obtain the size of the copied frame.
    fn get_num_bytes(&self) -> RVec<usize>;
    /// Used internally by Phaneron to get the size of the frame on the GPU.
    fn get_num_bytes_rgba(&self) -> usize;
    /// Used internally by Phaneron during the process of colour space conversion.
    fn get_total_bytes(&self) -> usize;
    /// Transforms a video frame into the described colour space / format.
    fn process_frame(
        &self,
        context: &crate::types::ProcessFrameContext,
        frame: crate::types::VideoFrame,
    ) -> crate::types::ConsumedVideoFrame;
    /// Copies a frame from the GPU.
    fn copy_frame(
        &self,
        context: &crate::types::FrameContext, // Required to prove that processing has finished
        frame: crate::types::ConsumedVideoFrame,
    ) -> RVec<RVec<u8>>;
}

/// A handle to a video frame that has been transformed into a requested colour space.
/// This handle is used to request a copy of the frame.
#[sabi_trait]
pub trait ConsumedVideoFrame {}

/// Provides functions for creating audio frames in 32 bit floating point format.
#[sabi_trait]
pub trait ToAudioF32: Send + Sync {
    /// Copies the input but does not transform it into the required format.
    fn load_frame(&self, input: &RSlice<u8>) -> crate::types::LoadedAudioFrame;
    /// Transforms a loaded audio frame into the common audio format.
    fn process_frame(&self, source: crate::types::LoadedAudioFrame) -> crate::types::AudioFrame;
}

/// A handle to an audio frame that has been copied into Phaneron's memory.
#[sabi_trait]
pub trait LoadedAudioFrame {}

/// Provides functions for consuming an audio frame in a prescribed format.
#[sabi_trait]
pub trait FromAudioF32: Send + Sync {
    /// Consumes an audio frame in the requested audio format.
    fn process_frame(
        &self,
        context: &crate::types::ProcessFrameContext,
        frame: crate::types::AudioFrame,
    ) -> crate::types::ConsumedAudioFrame;
    /// Provides an audio frame as a single buffer.
    fn copy_frame(
        &self,
        context: &crate::types::FrameContext,
        frame: crate::types::ConsumedAudioFrame,
    ) -> RVec<u8>;
}

/// Provides a handle to an audio frame that has been transformed into a requested format.
#[sabi_trait]
pub trait ConsumedAudioFrame {}
