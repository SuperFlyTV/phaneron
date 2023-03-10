use crate::{
    colour::ColourSpec,
    compute::{video_frame::VideoFrame, PhaneronComputeContext},
    load_save::{Loader, Saver},
    node_context::FrameContext,
};

pub struct LoadedVideoFrame {
    pub buffers: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>>,
    pub events: Vec<opencl3::event::Event>,
    pub width: usize,
    pub height: usize,
}

pub struct ConsumedVideoFrame {
    pub buffers: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>>,
    pub events: Vec<opencl3::event::Event>,
}

pub enum InterlaceMode {
    Progressive,
    TopField,
    BottomField,
}

impl InterlaceMode {
    pub fn as_kernel_param(&self) -> u32 {
        match self {
            InterlaceMode::Progressive => 0,
            InterlaceMode::TopField => 1,
            InterlaceMode::BottomField => 3,
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
        colour_spec: ColourSpec,
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

    pub fn get_num_bytes(&self) -> Vec<usize> {
        self.num_bytes.clone()
    }

    pub fn get_num_bytes_rgba(&self) -> usize {
        self.num_bytes_rgba
    }

    pub fn get_total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn load_frame(&self, inputs: &[&[u8]]) -> LoadedVideoFrame {
        let mut buffers: Vec<opencl3::memory::Buffer<opencl3::types::cl_uchar>> = vec![];
        let mut events: Vec<opencl3::event::Event> = vec![];

        for input in inputs {
            let (buffer, event) = self.context.load_frame_to_buffer(input);
            buffers.push(buffer);
            events.push(event);
        }

        LoadedVideoFrame {
            buffers,
            events,
            width: self.width,
            height: self.height,
        }
    }

    pub fn process_frame(&self, sources: LoadedVideoFrame) -> VideoFrame {
        self.loader.run(sources)
    }
}

unsafe impl Send for ToRGBA {}
unsafe impl Sync for ToRGBA {}

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
        colour_spec: ColourSpec,
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

    pub fn get_num_bytes(&self) -> Vec<usize> {
        self.num_bytes.clone()
    }

    pub fn get_num_bytes_rgba(&self) -> usize {
        self.num_bytes_rgba
    }

    pub fn get_total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub async fn copy_frame(
        &self,
        _context: &FrameContext, // Required to prove that processing has finished
        frame: ConsumedVideoFrame,
    ) -> Vec<Vec<u8>> {
        let mut buffers: Vec<Vec<u8>> = Vec::with_capacity(frame.buffers.len());

        for (i, buffer) in frame.buffers.iter().enumerate() {
            let mut out = vec![0u8; self.num_bytes[i]];
            self.context
                .copy_frame_from_buffer(buffer, &mut out, &frame.events);
            buffers.push(out);
        }

        buffers
    }

    pub async fn process_frame(&self, frame: VideoFrame) -> ConsumedVideoFrame {
        self.saver.run(frame)
    }
}

unsafe impl Send for FromRGBA {}
unsafe impl Sync for FromRGBA {}

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
