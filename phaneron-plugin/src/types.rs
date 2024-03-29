//! This module provides aliases for the trait objects generated by `abi_stable` for traits annotated with `sabi_trait`.

use abi_stable::std_types::{RArc, RBox};

pub type PhaneronPlugin = super::traits::PhaneronPlugin_TO<'static, RBox<()>>;
pub type Node = super::traits::Node_TO<'static, RBox<()>>;
pub type NodeContext = RArc<super::traits::NodeContext_TO<'static, RBox<()>>>;
pub type ProcessFrameContext = super::traits::ProcessFrameContext_TO<'static, RBox<()>>;
pub type FrameContext = super::traits::FrameContext_TO<'static, RBox<()>>;
pub type ProcessShader = super::traits::ProcessShader_TO<'static, RBox<()>>;
pub type NodeHandle = super::traits::NodeHandle_TO<'static, RBox<()>>;
pub type VideoFrame = RArc<super::traits::VideoFrame_TO<'static, RBox<()>>>;
pub type AudioFrame = RArc<super::traits::AudioFrame_TO<'static, RBox<()>>>;
pub type VideoOutput = super::traits::VideoOutput_TO<'static, RBox<()>>;
pub type AudioOutput = super::traits::AudioOutput_TO<'static, RBox<()>>;
pub type ToRGBA = super::traits::ToRGBA_TO<'static, RBox<()>>;
pub type LoadedVideoFrame = super::traits::LoadedVideoFrame_TO<'static, RBox<()>>;
pub type FromRGBA = super::traits::FromRGBA_TO<'static, RBox<()>>;
pub type ConsumedVideoFrame = super::traits::ConsumedVideoFrame_TO<'static, RBox<()>>;
pub type ToAudioF32 = super::traits::ToAudioF32_TO<'static, RBox<()>>;
pub type LoadedAudioFrame = super::traits::LoadedAudioFrame_TO<'static, RBox<()>>;
pub type FromAudioF32 = super::traits::FromAudioF32_TO<'static, RBox<()>>;
pub type ConsumedAudioFrame = super::traits::ConsumedAudioFrame_TO<'static, RBox<()>>;
