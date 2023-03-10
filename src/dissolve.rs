use crate::{
    compute::video_frame::{VideoFrame, VideoFrameId},
    node_context::NodeContext,
};

pub struct Dissolve {
    context: NodeContext,
    width: usize,
    height: usize,
    dissolve_cl: DissolveCl,
}

impl Dissolve {
    pub fn new(context: &NodeContext, width: usize, height: usize) -> Self {
        let dissolve_cl = DissolveCl::new(context, width, height);
        Self {
            context: context.clone(),
            width,
            height,
            dissolve_cl,
        }
    }

    pub fn run(&mut self, current: &VideoFrame, next: &VideoFrame, value: f32) -> Vec<VideoFrame> {
        let mut outputs: Vec<VideoFrame> = vec![];

        let output = self.dissolve_cl.run(&self.context, &[current, next], value);
        outputs.push(VideoFrame::new(
            VideoFrameId::default(),
            output,
            self.width,
            self.height,
        ));

        outputs
    }
}

pub struct DissolveCl {
    width: usize,
    height: usize,
    shader: opencl3::kernel::Kernel,
}

impl DissolveCl {
    fn new(context: &NodeContext, width: usize, height: usize) -> Self {
        let kernel = include_str!("../shaders/video_process/dissolve.cl");
        let shader = context.create_process_shader(kernel, "transition_dissolve");

        Self {
            width,
            height,
            shader,
        }
    }

    fn run(
        &self,
        context: &NodeContext,
        inputs: &[&VideoFrame; 2],
        value: f32,
    ) -> opencl3::memory::Image {
        let mut execute_kernel = opencl3::kernel::ExecuteKernel::new(&self.shader);
        let image_1 = &**inputs[0].get_buffer();
        let image_2 = &**inputs[1].get_buffer();
        let out = context.create_image(self.width, self.height);

        unsafe {
            execute_kernel
                .set_arg(image_1)
                .set_arg(image_2)
                .set_arg(&value)
                .set_arg(&out)
                .set_global_work_sizes(&[self.width, self.height])
        };

        context.run_process_shader(execute_kernel);

        out
    }
}
