use phaneron_plugin::{types::NodeContext, types::ProcessShader, types::VideoFrame, ShaderParams};

pub struct Dissolve {
    dissolve_cl: DissolveCl,
}

impl Dissolve {
    pub fn new(context: &NodeContext, width: usize, height: usize) -> Self {
        let dissolve_cl = DissolveCl::new(context, width, height);
        Self { dissolve_cl }
    }

    pub fn run(&mut self, current: &VideoFrame, next: &VideoFrame, value: f32) -> Vec<VideoFrame> {
        let mut outputs: Vec<VideoFrame> = vec![];

        let output = self.dissolve_cl.run(&[current, next], value);
        outputs.push(output);

        outputs
    }
}

pub struct DissolveCl {
    width: usize,
    height: usize,
    shader: ProcessShader,
}

impl DissolveCl {
    fn new(context: &NodeContext, width: usize, height: usize) -> Self {
        let kernel = include_str!("../shaders/dissolve.cl");
        let shader = context.create_process_shader(kernel.into(), "transition_dissolve".into());

        Self {
            width,
            height,
            shader,
        }
    }

    fn run(&self, inputs: &[&VideoFrame; 2], value: f32) -> VideoFrame {
        let mut params = ShaderParams::default();
        params.set_param_video_frame_input(inputs[0].clone());
        params.set_param_video_frame_input(inputs[1].clone());
        params.set_param_f32_input(value);
        params.set_param_video_frame_output(self.width, self.height);

        let outputs = self.shader.run(params, &[self.width, self.height]);

        outputs[0].clone()
    }
}
