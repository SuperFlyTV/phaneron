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

use abi_stable::{
    sabi_trait::{TD_CanDowncast, TD_Opaque},
    std_types::{RArc, RSlice, RVec},
};
use byteorder::{ByteOrder, LittleEndian};
use phaneron_plugin::{
    traits::AudioFrame_TO, traits::ConsumedAudioFrame_TO, traits::ConsumedVideoFrame_TO,
    traits::LoadedAudioFrame_TO, traits::LoadedVideoFrame_TO, traits::VideoFrame_TO,
    AudioChannelLayout, AudioFormat, ColourSpec,
};

use crate::{
    compute::{
        audio_frame::{AudioFrame, AudioFrameId},
        AsKernalParamU32, PhaneronComputeContext,
    },
    load_save::{Loader, Saver},
};

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct LoadedVideoFrame {
    pub buffers: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>>,
    pub events: Vec<opencl3::event::Event>,
    pub width: usize,
    pub height: usize,
}

impl phaneron_plugin::traits::LoadedVideoFrame for LoadedVideoFrame {}

pub struct ConsumedVideoFrame {
    pub buffers: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>>,
    pub events: Vec<opencl3::event::Event>,
}

impl phaneron_plugin::traits::ConsumedVideoFrame for ConsumedVideoFrame {}

impl AsKernalParamU32 for phaneron_plugin::InterlaceMode {
    fn as_kernel_param(&self) -> u32 {
        match self {
            phaneron_plugin::InterlaceMode::Progressive => 0,
            phaneron_plugin::InterlaceMode::TopField => 1,
            phaneron_plugin::InterlaceMode::BottomField => 3,
        }
    }
}

pub struct ToRGBA {
    context: PhaneronComputeContext,
    loader: Loader,
    num_bytes: Vec<usize>,
    num_bytes_rgba: usize,
    total_bytes: usize,
    width: usize,
    height: usize,
}

impl ToRGBA {
    pub fn new(
        context: PhaneronComputeContext,
        colour_spec: &ColourSpec,
        reader: Box<dyn Packer>,
    ) -> Self {
        let num_bytes = reader.get_num_bytes();
        let num_bytes_rgba = reader.get_num_bytes_rgba();
        let total_bytes = reader.get_total_bytes();
        let width = reader.get_width();
        let height = reader.get_height();
        let loader = Loader::new(context.clone(), colour_spec, reader);

        Self {
            context,
            loader,
            num_bytes,
            num_bytes_rgba,
            total_bytes,
            width,
            height,
        }
    }
}

impl phaneron_plugin::traits::ToRGBA for ToRGBA {
    fn get_num_bytes(&self) -> RVec<usize> {
        self.num_bytes.clone().into()
    }

    fn get_num_bytes_rgba(&self) -> usize {
        self.num_bytes_rgba
    }

    fn get_total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn load_frame(&self, inputs: &RSlice<RSlice<u8>>) -> phaneron_plugin::types::LoadedVideoFrame {
        let mut buffers: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>> = vec![];
        let mut events: Vec<opencl3::event::Event> = vec![];

        for input in inputs.as_slice() {
            let (buffer, event) = self.context.load_frame_to_buffer(input);
            buffers.push(buffer);
            events.push(event);
        }

        LoadedVideoFrame_TO::from_value(
            LoadedVideoFrame {
                buffers,
                events,
                width: self.width,
                height: self.height,
            },
            TD_CanDowncast,
        )
    }

    fn process_frame(
        &self,
        mut sources: phaneron_plugin::types::LoadedVideoFrame,
    ) -> phaneron_plugin::types::VideoFrame {
        let sources: LoadedVideoFrame =
            std::mem::take(sources.obj.downcast_as_mut::<LoadedVideoFrame>().unwrap());

        RArc::new(VideoFrame_TO::from_value(
            self.loader.run(sources),
            TD_Opaque,
        ))
    }
}

pub struct FromRGBA {
    context: PhaneronComputeContext,
    saver: Saver,
    num_bytes: Vec<usize>,
    num_bytes_rgba: usize,
    total_bytes: usize,
}

impl FromRGBA {
    pub fn new(
        context: PhaneronComputeContext,
        colour_spec: &ColourSpec,
        writer: Box<dyn Unpacker>,
    ) -> Self {
        let num_bytes = writer.get_num_bytes();
        let num_bytes_rgba = writer.get_num_bytes_rgba();
        let total_bytes = writer.get_total_bytes();
        let saver = Saver::new(context.clone(), colour_spec, writer);

        Self {
            context,
            saver,
            num_bytes,
            num_bytes_rgba,
            total_bytes,
        }
    }
}

impl phaneron_plugin::traits::FromRGBA for FromRGBA {
    fn get_num_bytes(&self) -> RVec<usize> {
        self.num_bytes.clone().into()
    }

    fn get_num_bytes_rgba(&self) -> usize {
        self.num_bytes_rgba
    }

    fn get_total_bytes(&self) -> usize {
        self.total_bytes
    }

    fn copy_frame(
        &self,
        _context: &phaneron_plugin::types::FrameContext, // Required to prove that processing has finished
        frame: phaneron_plugin::types::ConsumedVideoFrame,
    ) -> RVec<RVec<u8>> {
        let consumed_video_frame = frame.obj.downcast_into::<ConsumedVideoFrame>().unwrap();
        let mut buffers: RVec<RVec<u8>> = RVec::with_capacity(consumed_video_frame.buffers.len());

        for (i, buffer) in consumed_video_frame.buffers.iter().enumerate() {
            let mut out = vec![0u8; self.num_bytes[i]];
            self.context
                .copy_frame_from_buffer(buffer, &mut out, &consumed_video_frame.events);
            buffers.push(out.into());
        }

        buffers
    }

    fn process_frame(
        &self,
        _context: &phaneron_plugin::types::ProcessFrameContext,
        frame: phaneron_plugin::types::VideoFrame,
    ) -> phaneron_plugin::types::ConsumedVideoFrame {
        ConsumedVideoFrame_TO::from_value(self.saver.run(frame), TD_CanDowncast)
    }
}

pub trait Packer: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_kernel(&self) -> &str;
    fn get_width(&self) -> usize;
    fn get_height(&self) -> usize;
    fn get_num_bits(&self) -> usize;
    fn get_luma_black(&self) -> f32;
    fn get_luma_white(&self) -> f32;
    fn get_chroma_range(&self) -> f32;
    fn get_num_bytes(&self) -> Vec<usize>;
    fn get_num_bytes_rgba(&self) -> usize;
    fn get_is_rgb(&self) -> bool;
    fn get_total_bytes(&self) -> usize;
    fn get_work_items_per_group(&self) -> usize;
    fn get_global_work_items(&self) -> usize;
    fn get_kernel_params(
        &self,
        kernel: &mut opencl3::kernel::ExecuteKernel,
        inputs: &[&opencl3::memory::Buffer<opencl3::types::cl_uchar>],
        output: &mut opencl3::memory::Buffer<opencl3::types::cl_uchar>,
    );
}

pub trait Unpacker: Send + Sync {
    fn get_name(&self) -> &str;
    fn get_kernel(&self) -> &str;
    fn get_width(&self) -> usize;
    fn get_height(&self) -> usize;
    fn get_num_bits(&self) -> usize;
    fn get_luma_black(&self) -> f32;
    fn get_luma_white(&self) -> f32;
    fn get_chroma_range(&self) -> f32;
    fn get_num_bytes(&self) -> Vec<usize>;
    fn get_num_bytes_rgba(&self) -> usize;
    fn get_is_rgb(&self) -> bool;
    fn get_total_bytes(&self) -> usize;
    fn get_work_items_per_group(&self) -> usize;
    fn get_global_work_items(&self) -> usize;
    fn get_kernel_params(
        &self,
        kernel: &mut opencl3::kernel::ExecuteKernel,
        input: &opencl3::memory::Buffer<opencl3::types::cl_uchar>,
        outputs: &mut Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>>,
    );
}

pub struct LoadedAudioFrame {
    audio: Vec<u8>,
}

impl phaneron_plugin::traits::LoadedAudioFrame for LoadedAudioFrame {}

#[derive(Default)]
pub struct ConsumedAudioFrame {
    buffer: Vec<u8>,
}

impl phaneron_plugin::traits::ConsumedAudioFrame for ConsumedAudioFrame {}

pub struct ToAudioF32 {
    audio_format: AudioFormat,
    channel_layout: AudioChannelLayout,
}

impl ToAudioF32 {
    pub fn new(audio_format: AudioFormat, channel_layout: AudioChannelLayout) -> Self {
        Self {
            audio_format,
            channel_layout,
        }
    }
}

impl phaneron_plugin::traits::ToAudioF32 for ToAudioF32 {
    fn load_frame(&self, input: &RSlice<u8>) -> phaneron_plugin::types::LoadedAudioFrame {
        LoadedAudioFrame_TO::from_value(
            LoadedAudioFrame {
                audio: input.to_vec(),
            },
            TD_CanDowncast,
        )
    }

    fn process_frame(
        &self,
        source: phaneron_plugin::types::LoadedAudioFrame,
    ) -> phaneron_plugin::types::AudioFrame {
        let num_channels: usize = match self.channel_layout {
            AudioChannelLayout::Mono => 1,
            AudioChannelLayout::L => 1,
            AudioChannelLayout::R => 1,
            AudioChannelLayout::L_R => 2,
            AudioChannelLayout::R_L => 2,
        };
        let bytes_per_sample: usize = match self.audio_format {
            AudioFormat::I16 => 2,
            AudioFormat::U16 => 2,
            AudioFormat::I32 => 4,
            AudioFormat::F32 => 4,
        };

        let frame = source.obj.downcast_into::<LoadedAudioFrame>().unwrap();
        let mut processed_buffers: Vec<Vec<f32>> = Vec::with_capacity(num_channels);
        for i in 0..num_channels {
            let buffer: Vec<u8> = frame
                .audio
                .iter()
                .skip(i * bytes_per_sample)
                .step_by((bytes_per_sample * i).max(1))
                .copied()
                .collect();
            match self.audio_format {
                AudioFormat::I16 => {
                    let mut grouped_sample_buffer: Vec<i16> =
                        vec![0i16; (buffer.len() / num_channels) / bytes_per_sample];
                    LittleEndian::read_i16_into(&buffer, &mut grouped_sample_buffer);
                    let processed_buffer: Vec<f32> = grouped_sample_buffer
                        .iter()
                        .map(|sample| ((*sample as f64) / i16::MAX as f64) as f32)
                        .collect();
                    processed_buffers.push(processed_buffer);
                }
                AudioFormat::U16 => {
                    let mut grouped_sample_buffer: Vec<u16> =
                        vec![0u16; (buffer.len() / num_channels) / bytes_per_sample];
                    LittleEndian::read_u16_into(&buffer, &mut grouped_sample_buffer);
                    let processed_buffer: Vec<f32> = grouped_sample_buffer
                        .iter()
                        .map(|sample| (((*sample as f64) / u16::MAX as f64) * 2.0 - 1.0) as f32)
                        .collect();
                    processed_buffers.push(processed_buffer);
                }
                AudioFormat::I32 => {
                    let mut grouped_sample_buffer: Vec<i32> =
                        vec![0i32; (buffer.len() / num_channels) / bytes_per_sample];
                    LittleEndian::read_i32_into(&buffer, &mut grouped_sample_buffer);
                    let processed_buffer: Vec<f32> = grouped_sample_buffer
                        .iter()
                        .map(|sample| ((*sample as f64) / i32::MAX as f64) as f32)
                        .collect();
                    processed_buffers.push(processed_buffer);
                }
                AudioFormat::F32 => {
                    let mut grouped_sample_buffer: Vec<f32> =
                        vec![0f32; (buffer.len() / num_channels) / bytes_per_sample];
                    LittleEndian::read_f32_into(&buffer, &mut grouped_sample_buffer);
                    processed_buffers.push(grouped_sample_buffer);
                }
            }
        }

        RArc::new(AudioFrame_TO::from_value(
            AudioFrame::new(AudioFrameId::default(), processed_buffers),
            TD_CanDowncast,
        ))
    }
}

pub struct FromAudioF32 {
    audio_format: AudioFormat,
    channel_layout: AudioChannelLayout,
}

impl FromAudioF32 {
    pub fn new(audio_format: AudioFormat, channel_layout: AudioChannelLayout) -> Self {
        Self {
            audio_format,
            channel_layout,
        }
    }
}

impl phaneron_plugin::traits::FromAudioF32 for FromAudioF32 {
    fn process_frame(
        &self,
        _context: &phaneron_plugin::types::ProcessFrameContext,
        frame: phaneron_plugin::types::AudioFrame,
    ) -> phaneron_plugin::types::ConsumedAudioFrame {
        let num_channels: usize = match self.channel_layout {
            AudioChannelLayout::Mono => 1,
            AudioChannelLayout::L => 1,
            AudioChannelLayout::R => 1,
            AudioChannelLayout::L_R => 2,
            AudioChannelLayout::R_L => 2,
        };

        if frame.buffers().len() != num_channels {
            todo!("Return a reasonable error")
        }

        let num_bytes: usize = match self.audio_format {
            AudioFormat::I16 => 2,
            AudioFormat::U16 => 2,
            AudioFormat::I32 => 4,
            AudioFormat::F32 => 4,
        };

        let num_samples = frame.buffers().get(0).unwrap().len();

        let mut bytes = vec![0u8; num_bytes];
        let mut output_buffer = vec![0u8; num_channels * num_bytes * num_samples];

        for sample_index in 0..num_samples {
            for channel_index in 0..num_channels {
                let sample = &frame.buffers()[channel_index][sample_index];
                match self.audio_format {
                    AudioFormat::I16 => {
                        let sample = f64::round(*sample as f64 * i16::MAX as f64) as i16;
                        LittleEndian::write_i16_into(&[sample], &mut bytes);
                    }
                    AudioFormat::U16 => {
                        let sample = (sample + 1.0) / 2.0;
                        let sample = f64::round(sample as f64 * u16::MAX as f64) as u16;
                        LittleEndian::write_u16_into(&[sample], &mut bytes);
                    }
                    AudioFormat::F32 => {
                        LittleEndian::write_f32_into(&[*sample], &mut bytes);
                    }
                    AudioFormat::I32 => {
                        let sample = f64::round(*sample as f64 * i32::MAX as f64) as i32;
                        LittleEndian::write_i32_into(&[sample], &mut bytes);
                    }
                }
                let pos: usize =
                    (sample_index * num_channels * num_bytes) + (channel_index * num_bytes);
                output_buffer[pos..pos + num_bytes].copy_from_slice(&bytes);
            }
        }

        ConsumedAudioFrame_TO::from_value(
            ConsumedAudioFrame {
                buffer: output_buffer,
            },
            TD_CanDowncast,
        )
    }

    fn copy_frame(
        &self,
        _context: &phaneron_plugin::types::FrameContext,
        mut frame: phaneron_plugin::types::ConsumedAudioFrame,
    ) -> RVec<u8> {
        let frame = std::mem::take(frame.obj.downcast_as_mut::<ConsumedAudioFrame>().unwrap());
        frame.buffer.into()
    }
}
