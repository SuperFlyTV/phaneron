use crate::{
    colour::{
        common_space_to_rgb_matrix, gamma_to_linear_lut, linear_to_gamma_lut,
        rgb_to_common_space_matrix, rgb_to_ycbcr_matrix, ycbcr_to_rgb_matrix, ColourSpec,
    },
    compute::{
        video_frame::{VideoFrame, VideoFrameId},
        PhaneronComputeContext,
    },
    io::{ConsumedVideoFrame, LoadedVideoFrame, Packer, Unpacker},
};

pub struct Loader {
    context: PhaneronComputeContext,
    packer: Box<dyn Packer>,
    shader: opencl3::kernel::Kernel,
    gamma_lut: opencl3::memory::Buffer<opencl3::types::cl_float>,
    gamut_matrix: opencl3::memory::Buffer<opencl3::types::cl_float>,
    yuv_to_rgb_matrix: Option<opencl3::memory::Buffer<opencl3::types::cl_float>>,
}

impl Loader {
    pub fn new(
        context: PhaneronComputeContext,
        colour_spec: ColourSpec,
        packer: Box<dyn Packer>,
    ) -> Self {
        let rgb = packer.get_is_rgb();
        let gamma_lut = gamma_to_linear_lut(&colour_spec);
        let gamut_matrix = rgb_to_common_space_matrix(&colour_spec);
        let gamut_matrix = gamut_matrix
            .data
            .0
            .iter()
            .cloned()
            .flatten()
            .collect::<Vec<f32>>();

        let kernel = packer.get_kernel();

        let yuv_to_rgb_matrix = if rgb {
            None
        } else {
            let yuv_to_rgb_matrix = ycbcr_to_rgb_matrix(
                &colour_spec,
                packer.get_num_bits(),
                packer.get_luma_black(),
                packer.get_luma_white(),
                packer.get_chroma_range(),
            );
            let yuv_to_rgb_matrix = yuv_to_rgb_matrix
                .data
                .0
                .iter()
                .cloned()
                .flatten()
                .collect::<Vec<f32>>();
            Some(context.create_loadsave_params_buffer(&yuv_to_rgb_matrix))
        };

        let shader = context.create_load_shader(kernel);
        let gamma_lut = context.create_loadsave_params_buffer(&gamma_lut);
        let gamut_matrix = context.create_loadsave_params_buffer(&gamut_matrix);

        Self {
            context,
            packer,
            shader,
            gamma_lut,
            gamut_matrix,
            yuv_to_rgb_matrix,
        }
    }

    pub fn run(&self, source: LoadedVideoFrame) -> VideoFrame {
        let mut execute_kernel = opencl3::kernel::ExecuteKernel::new(&self.shader);
        let mut dest = self
            .context
            .create_video_frame_buffer(self.packer.get_num_bytes_rgba());

        self.packer.get_kernel_params(
            &mut execute_kernel,
            &source
                .buffers
                .iter()
                .collect::<Vec<&opencl3::memory::Buffer<opencl3::types::cl_uchar>>>(),
            &mut dest,
        );

        if let Some(yuv_to_rgb_matrix) = &self.yuv_to_rgb_matrix {
            unsafe {
                execute_kernel.set_arg(yuv_to_rgb_matrix);
            }
        }

        unsafe {
            execute_kernel
                .set_arg(&self.gamma_lut)
                .set_arg(&self.gamut_matrix);
        }

        execute_kernel
            .set_local_work_size(self.packer.get_work_items_per_group())
            .set_global_work_size(self.packer.get_global_work_items());

        self.context
            .run_loadsave_shader(execute_kernel, &source.events);

        let out = self.context.create_image_from_buffer(
            self.packer.get_width(),
            self.packer.get_height(),
            &dest,
        );

        VideoFrame::new(
            VideoFrameId::default(),
            out,
            self.packer.get_width(),
            self.packer.get_height(),
        )
    }
}

unsafe impl Send for Loader {}
unsafe impl Sync for Loader {}

pub struct Saver {
    context: PhaneronComputeContext,
    unpacker: Box<dyn Unpacker>,
    shader: opencl3::kernel::Kernel,
    num_bytes: Vec<usize>,
    gamma_lut: opencl3::memory::Buffer<opencl3::types::cl_float>,
    gamut_matrix: opencl3::memory::Buffer<opencl3::types::cl_float>,
    rgb_to_yuv_matrix: Option<opencl3::memory::Buffer<opencl3::types::cl_float>>,
}

impl Saver {
    pub fn new(
        context: PhaneronComputeContext,
        colour_spec: ColourSpec,
        unpacker: Box<dyn Unpacker>,
    ) -> Self {
        let rgb = unpacker.get_is_rgb();
        let gamma_lut = linear_to_gamma_lut(&colour_spec);
        let gamut_matrix = common_space_to_rgb_matrix(&colour_spec);
        let gamut_matrix = gamut_matrix
            .data
            .0
            .iter()
            .cloned()
            .flatten()
            .collect::<Vec<f32>>();
        let kernel = unpacker.get_kernel();
        let num_bytes = unpacker.get_num_bytes();

        let rgb_to_yuv_matrix = if rgb {
            None
        } else {
            let yuv_to_rgb_matrix = rgb_to_ycbcr_matrix(
                &colour_spec,
                unpacker.get_num_bits(),
                unpacker.get_luma_black(),
                unpacker.get_luma_white(),
                unpacker.get_chroma_range(),
            );
            let yuv_to_rgb_matrix = yuv_to_rgb_matrix
                .data
                .0
                .iter()
                .cloned()
                .flatten()
                .collect::<Vec<f32>>();
            Some(context.create_loadsave_params_buffer(&yuv_to_rgb_matrix))
        };

        let shader = context.create_save_shader(kernel);
        let gamma_lut = context.create_loadsave_params_buffer(&gamma_lut);
        let gamut_matrix = context.create_loadsave_params_buffer(&gamut_matrix);

        Self {
            context,
            unpacker,
            shader,
            num_bytes,
            gamma_lut,
            gamut_matrix,
            rgb_to_yuv_matrix,
        }
    }

    pub fn run(&self, source: VideoFrame) -> ConsumedVideoFrame {
        let mut execute_kernel = opencl3::kernel::ExecuteKernel::new(&self.shader);
        let mut dests: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>> =
            Vec::with_capacity(self.num_bytes.len());

        for dest_size in self.num_bytes.iter() {
            dests.push(self.context.create_buffer(*dest_size));
        }

        let buffer = self.context.create_buffer_from_image(
            self.unpacker.get_width(),
            self.unpacker.get_height(),
            self.unpacker.get_num_bytes_rgba(),
            source.get_buffer(),
        );
        self.unpacker
            .get_kernel_params(&mut execute_kernel, &buffer, &mut dests);

        if let Some(rgb_to_yuv_matrix) = &self.rgb_to_yuv_matrix {
            unsafe { execute_kernel.set_arg(rgb_to_yuv_matrix) };
        }

        unsafe {
            execute_kernel.set_arg(&self.gamma_lut);
        }

        execute_kernel
            // .set_arg(&self.gamut_matrix) // TODO: Colour space transforms
            .set_local_work_size(self.unpacker.get_work_items_per_group())
            .set_global_work_size(self.unpacker.get_global_work_items());

        let save_event = self.context.run_loadsave_shader(execute_kernel, &[]); // TODO: Events

        ConsumedVideoFrame {
            buffers: dests,
            events: vec![save_event],
        }
    }
}

unsafe impl Send for Saver {}
unsafe impl Sync for Saver {}
