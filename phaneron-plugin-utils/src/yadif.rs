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

use std::collections::VecDeque;

use phaneron_plugin::{traits::ProcessShader, types::NodeContext, types::VideoFrame, ShaderParams};

#[derive(PartialEq, Eq)]
pub enum YadifMode {
    Field,
    FrameNospatial,
    FieldNospatial,
}

pub struct YadifConfig {
    pub mode: YadifMode,
    pub tff: bool,
}

pub struct Yadif {
    config: YadifConfig,
    send_field: bool,
    skip_spatial: bool,
    yadif_cl: YadifCl,
    input: VecDeque<VideoFrame>,
}

impl Yadif {
    pub fn new(context: &NodeContext, width: usize, height: usize, config: YadifConfig) -> Self {
        let send_field =
            config.mode == YadifMode::Field || config.mode == YadifMode::FieldNospatial;
        let skip_spatial =
            config.mode == YadifMode::FrameNospatial || config.mode == YadifMode::FieldNospatial;
        let yadif_cl = YadifCl::new(context, width, height);
        Self {
            config,
            send_field,
            skip_spatial,
            yadif_cl,
            input: VecDeque::with_capacity(4), // 3 fields + last one pushed
        }
    }

    pub fn run(&mut self, source: &VideoFrame) -> Vec<VideoFrame> {
        self.input.push_front(source.clone());
        if self.input.len() < 3 {
            return vec![];
        }

        if self.input.len() > 3 {
            self.input.pop_back();
        }

        let mut outputs: Vec<VideoFrame> = vec![];

        let output = self.run_yadif(false);
        outputs.push(output);

        if self.send_field {
            let output = self.run_yadif(true);
            outputs.push(output);
        }

        outputs
    }

    fn run_yadif(&mut self, is_second: bool) -> VideoFrame {
        self.yadif_cl.run(
            self.input.iter().collect::<Vec<&VideoFrame>>().as_slice(),
            u32::from(self.config.tff) ^ u32::from(!is_second),
            u32::from(self.config.tff),
            u32::from(self.skip_spatial),
        )
    }
}

pub struct YadifCl {
    width: usize,
    height: usize,
    shader: Box<dyn ProcessShader>,
}

impl YadifCl {
    fn new(context: &NodeContext, width: usize, height: usize) -> Self {
        let kernel = include_str!("shaders/yadif.cl");
        let shader = context.create_process_shader(kernel.into(), "yadif".into());

        Self {
            width,
            height,
            shader: Box::new(shader),
        }
    }

    fn run(&self, inputs: &[&VideoFrame], parity: u32, tff: u32, skip_spatial: u32) -> VideoFrame {
        let mut params = ShaderParams::default();
        params.set_param_video_frame_input(inputs[0].clone());
        params.set_param_video_frame_input(inputs[1].clone());
        params.set_param_video_frame_input(inputs[2].clone());
        params.set_param_u32_input(parity);
        params.set_param_u32_input(tff);
        params.set_param_u32_input(skip_spatial);
        params.set_param_video_frame_output(self.width, self.height);

        let outputs = self.shader.run(params, &[self.width, self.height]);

        outputs[0].clone()
    }
}
