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

use phaneron_plugin::InterlaceMode;

use crate::{
    compute::AsKernalParamU32,
    io::{Packer, Unpacker},
};

const PIXELS_PER_WORK_ITEM: f32 = 64.0;

fn get_pitch(width: usize) -> usize {
    width + 7 - ((width - 1) % 8)
}

fn get_pitch_bytes(width: usize) -> usize {
    get_pitch(width) * 2
}

pub struct YUV422p10Reader {
    width: usize,
    height: usize,
    num_bytes: Vec<usize>,
    work_items_per_group: usize,
    global_work_items: usize,
}

impl YUV422p10Reader {
    pub fn new(width: usize, height: usize) -> Self {
        let luma_bytes = get_pitch_bytes(width) * height;
        let pitch = get_pitch(width) as f32;
        let work_items_per_group = f32::ceil(pitch / PIXELS_PER_WORK_ITEM) as usize;
        let global_work_items = work_items_per_group * height;

        Self {
            width,
            height,
            num_bytes: vec![luma_bytes, luma_bytes / 2, luma_bytes / 2],
            work_items_per_group,
            global_work_items,
        }
    }
}

impl Packer for YUV422p10Reader {
    fn get_name(&self) -> &str {
        "YUV422p10 Reader"
    }

    fn get_kernel(&self) -> &str {
        include_str!("../../shaders/video_process/load/yuv422p10.cl")
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }

    fn get_num_bits(&self) -> usize {
        10
    }

    fn get_luma_black(&self) -> f32 {
        64.0
    }

    fn get_luma_white(&self) -> f32 {
        940.0
    }

    fn get_chroma_range(&self) -> f32 {
        896.0
    }

    fn get_num_bytes(&self) -> Vec<usize> {
        self.num_bytes.clone()
    }

    fn get_num_bytes_rgba(&self) -> usize {
        self.width * self.height * 4 * 4
    }

    fn get_is_rgb(&self) -> bool {
        false
    }

    fn get_total_bytes(&self) -> usize {
        self.num_bytes.iter().sum()
    }

    fn get_work_items_per_group(&self) -> usize {
        self.work_items_per_group
    }

    fn get_global_work_items(&self) -> usize {
        self.global_work_items
    }

    fn get_kernel_params(
        &self,
        kernel: &mut opencl3::kernel::ExecuteKernel,
        inputs: &[&opencl3::memory::Buffer<opencl3::types::cl_uchar>],
        output: &mut opencl3::memory::Buffer<opencl3::types::cl_uchar>,
    ) {
        if inputs.len() != 3 {
            panic!(
                "Reader for {} requires exactly 3 inputs, received {}",
                self.get_name(),
                inputs.len()
            );
        }

        let width = self.width as u32;

        unsafe {
            kernel
                .set_arg(inputs[0])
                .set_arg(inputs[1])
                .set_arg(inputs[2])
                .set_arg(output)
                .set_arg(&width)
        };
    }
}

pub struct YUV422p10Writer {
    width: usize,
    height: usize,
    num_bytes: Vec<usize>,
    interlace: InterlaceMode,
    work_items_per_group: usize,
    global_work_items: usize,
}

impl YUV422p10Writer {
    pub fn new(width: usize, height: usize, interlace: InterlaceMode) -> Self {
        let luma_bytes = get_pitch_bytes(width) * height;
        let pitch = get_pitch(width) as f32;
        let work_items_per_group = f32::ceil(pitch / PIXELS_PER_WORK_ITEM) as usize;
        let global_work_items = (work_items_per_group * height)
            / match interlace {
                InterlaceMode::Progressive => 1,
                _ => 2,
            };

        Self {
            width,
            height,
            num_bytes: vec![luma_bytes, luma_bytes / 2, luma_bytes / 2],
            interlace,
            work_items_per_group,
            global_work_items,
        }
    }
}

impl Unpacker for YUV422p10Writer {
    fn get_name(&self) -> &str {
        "YUV422p10 Writer"
    }

    fn get_kernel(&self) -> &str {
        include_str!("../../shaders/video_process/consume/yuv422p10.cl")
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }

    fn get_num_bits(&self) -> usize {
        10
    }

    fn get_luma_black(&self) -> f32 {
        64.0
    }

    fn get_luma_white(&self) -> f32 {
        940.0
    }

    fn get_chroma_range(&self) -> f32 {
        896.0
    }

    fn get_num_bytes(&self) -> Vec<usize> {
        self.num_bytes.clone()
    }

    fn get_num_bytes_rgba(&self) -> usize {
        self.width * self.height * 4 * 4
    }

    fn get_is_rgb(&self) -> bool {
        false
    }

    fn get_total_bytes(&self) -> usize {
        self.num_bytes.iter().sum()
    }

    fn get_work_items_per_group(&self) -> usize {
        self.work_items_per_group
    }

    fn get_global_work_items(&self) -> usize {
        self.global_work_items
    }

    fn get_kernel_params(
        &self,
        kernel: &mut opencl3::kernel::ExecuteKernel,
        input: &opencl3::memory::Buffer<opencl3::types::cl_uchar>,
        outputs: &mut Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>>,
    ) {
        if outputs.len() != 3 {
            panic!(
                "Reader for {} requires exactly 3 outputs, received {}",
                self.get_name(),
                outputs.len()
            );
        }

        let width = self.width as u32;

        unsafe {
            kernel
                .set_arg(input)
                .set_arg(&outputs[0])
                .set_arg(&outputs[1])
                .set_arg(&outputs[2])
                .set_arg(&width)
                .set_arg(&self.interlace.as_kernel_param())
        };
    }
}
