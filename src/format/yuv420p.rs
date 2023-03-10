use crate::io::{InterlaceMode, Packer, Unpacker};

const PIXELS_PER_WORK_ITEM: f32 = 64.0;

fn get_pitch(width: usize) -> usize {
    width + 7 - ((width - 1) % 8)
}

fn get_pitch_bytes(width: usize) -> usize {
    get_pitch(width)
}

pub struct YUV420pReader {
    width: usize,
    height: usize,
    num_bytes: Vec<usize>,
    work_items_per_group: usize,
    global_work_items: usize,
}

impl YUV420pReader {
    pub fn new(width: usize, height: usize) -> Self {
        let luma_bytes = get_pitch_bytes(width) * height;
        let pitch = get_pitch(width) as f32;
        let work_items_per_group = f32::ceil(pitch / PIXELS_PER_WORK_ITEM) as usize;
        let global_work_items = (work_items_per_group * height) / 2;

        Self {
            width,
            height,
            num_bytes: vec![luma_bytes, luma_bytes / 4, luma_bytes / 4],
            work_items_per_group,
            global_work_items,
        }
    }
}

impl Packer for YUV420pReader {
    fn get_name(&self) -> &str {
        "YUV420p Reader"
    }

    fn get_kernel(&self) -> &str {
        include_str!("../../shaders/video_process/load/yuv420p.cl")
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }

    fn get_num_bits(&self) -> usize {
        8
    }

    fn get_luma_black(&self) -> f32 {
        16.0
    }

    fn get_luma_white(&self) -> f32 {
        235.0
    }

    fn get_chroma_range(&self) -> f32 {
        224.0
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

pub struct YUV420pWriter {
    width: usize,
    height: usize,
    num_bytes: Vec<usize>,
    interlace: InterlaceMode,
    work_items_per_group: usize,
    global_work_items: usize,
}

impl YUV420pWriter {
    pub fn new(width: usize, height: usize, interlace: InterlaceMode) -> Self {
        let luma_bytes = get_pitch_bytes(width) * height;
        let pitch = get_pitch(width) as f32;
        let work_items_per_group = f32::ceil(pitch / PIXELS_PER_WORK_ITEM) as usize;
        let global_work_items = (work_items_per_group * height) / 2;

        Self {
            width,
            height,
            num_bytes: vec![luma_bytes, luma_bytes / 4, luma_bytes / 4],
            interlace,
            work_items_per_group,
            global_work_items,
        }
    }
}

impl Unpacker for YUV420pWriter {
    fn get_name(&self) -> &str {
        "YUV420p Writer"
    }

    fn get_kernel(&self) -> &str {
        include_str!("../../shaders/video_process/consume/yuv420p.cl")
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }

    fn get_num_bits(&self) -> usize {
        8
    }

    fn get_luma_black(&self) -> f32 {
        16.0
    }

    fn get_luma_white(&self) -> f32 {
        235.0
    }

    fn get_chroma_range(&self) -> f32 {
        224.0
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
