use std::collections::VecDeque;

use crate::{
    compute::video_frame::{VideoFrame, VideoFrameId},
    node_context::NodeContext,
};

#[derive(PartialEq, Eq)]
pub enum YadifMode {
    Frame,
    Field,
    FrameNospatial,
    FieldNospatial,
}

pub struct YadifConfig {
    pub mode: YadifMode,
    pub tff: bool,
}

pub struct Yadif {
    context: NodeContext,
    width: usize,
    height: usize,
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
            context: context.clone(),
            width,
            height,
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
        outputs.push(VideoFrame::new(
            VideoFrameId::default(),
            output,
            self.width,
            self.height,
        ));

        if self.send_field {
            let output = self.run_yadif(true);
            outputs.push(VideoFrame::new(
                VideoFrameId::default(),
                output,
                self.width,
                self.height,
            ));
        }

        outputs
    }

    fn run_yadif(&mut self, is_second: bool) -> opencl3::memory::Image {
        self.yadif_cl.run(
            &self.context,
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
    shader: opencl3::kernel::Kernel,
}

impl YadifCl {
    fn new(context: &NodeContext, width: usize, height: usize) -> Self {
        let kernel = include_str!("../shaders/video_process/yadif.cl");
        let shader = context.create_process_shader(kernel, "yadif");

        Self {
            width,
            height,
            shader,
        }
    }

    fn run(
        &self,
        context: &NodeContext,
        inputs: &[&VideoFrame],
        parity: u32,
        tff: u32,
        skip_spatial: u32,
    ) -> opencl3::memory::Image {
        let mut execute_kernel = opencl3::kernel::ExecuteKernel::new(&self.shader);
        let image_1 = &**inputs[0].get_buffer();
        let image_2 = &**inputs[1].get_buffer();
        let image_3 = &**inputs[2].get_buffer();
        let out = context.create_image(self.width, self.height);

        unsafe {
            execute_kernel
                .set_arg(image_1)
                .set_arg(image_2)
                .set_arg(image_3)
                .set_arg(&parity)
                .set_arg(&tff)
                .set_arg(&skip_spatial)
                .set_arg(&out)
                .set_global_work_sizes(&[self.width, self.height])
        };

        context.run_process_shader(execute_kernel);

        out
    }
}
