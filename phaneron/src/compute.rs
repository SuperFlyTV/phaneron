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

use std::{ptr, sync::Arc};

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{RArc, RVec},
};
use opencl3::{
    memory::{CL_FLOAT, CL_MEM_OBJECT_IMAGE2D, CL_RGBA},
    types::{cl_image_desc, cl_image_format},
};
use phaneron_plugin::{traits::ProcessShader_TO, traits::VideoFrame_TO, ShaderParam, ShaderParams};
use tracing::debug;

use self::video_frame::{VideoFrame, VideoFrameId};

pub mod audio_frame;
pub mod audio_output;
pub mod video_frame;
pub mod video_output;

pub trait AsKernalParamU32 {
    fn as_kernel_param(&self) -> u32;
}

#[cfg(Debug)]
fn cl_queue_properties() -> opencl3::types::cl_command_queue_properties {
    opencl3::command_queue::CL_QUEUE_PROFILING_ENABLE
}

fn cl_queue_properties() -> opencl3::types::cl_command_queue_properties {
    0
}

pub async fn create_compute_context() -> PhaneronComputeContext {
    // Find a usable device for this application
    let device_id = *opencl3::device::get_all_devices(opencl3::device::CL_DEVICE_TYPE_GPU)
        .unwrap()
        .first()
        .expect("no device found in platform");
    let device = opencl3::device::Device::new(device_id);
    let extensions = device.extensions().unwrap();
    debug!("Device extensions: {}", extensions);

    // Create a Context on an OpenCL device
    let cl_context =
        opencl3::context::Context::from_device(&device).expect("Context::from_device failed");

    // Create a command_queue on the Context's device
    let load_queue = unsafe {
        opencl3::command_queue::CommandQueue::create_with_properties(
            &cl_context,
            cl_context.default_device(),
            cl_queue_properties(),
            0,
        )
        .expect("CommandQueue::create failed")
    };
    let process_queue = unsafe {
        opencl3::command_queue::CommandQueue::create_with_properties(
            &cl_context,
            cl_context.default_device(),
            cl_queue_properties(),
            0,
        )
        .expect("CommandQueue::create failed")
    };
    let unload_queue = unsafe {
        opencl3::command_queue::CommandQueue::create_with_properties(
            &cl_context,
            cl_context.default_device(),
            cl_queue_properties(),
            0,
        )
        .expect("CommandQueue::create failed")
    };

    let (buffer_drop_event_tx, mut buffer_drop_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let inner_context = PhaneronComputeContextInner {
        cl_context: std::sync::Mutex::new(cl_context),
        load_queue: std::sync::Mutex::new(load_queue),
        process_queue: std::sync::Mutex::new(process_queue),
        unload_queue: std::sync::Mutex::new(unload_queue),
        video_buffers: Default::default(),
        buffer_drop_event_tx,
    };
    let inner_context = Arc::new(inner_context);

    let dropper_context = inner_context.clone();
    tokio::spawn(async move {
        while let Some(buffer_index) = buffer_drop_event_rx.recv().await {
            let mut buffers = dropper_context.video_buffers.lock().unwrap();
            buffers.get_mut(buffer_index).unwrap().available = true;
        }
    });

    PhaneronComputeContext {
        inner: inner_context,
    }
}

pub struct PhaneronComputeContext {
    inner: Arc<PhaneronComputeContextInner>,
}

impl PhaneronComputeContext {
    pub fn load_frame_to_buffer(
        &self,
        data: &[u8],
    ) -> (
        opencl3::memory::Buffer<opencl3::types::cl_uchar>,
        opencl3::event::Event,
    ) {
        let context = self.inner.cl_context.lock().unwrap();
        let mut buf = unsafe {
            opencl3::memory::Buffer::<opencl3::types::cl_uchar>::create(
                &context,
                opencl3::memory::CL_MEM_READ_ONLY,
                data.len(),
                ptr::null_mut(),
            )
            .unwrap()
        };
        let queue = self.inner.load_queue.lock().unwrap();
        let load_frame_event = unsafe {
            queue
                .enqueue_write_buffer(&mut buf, opencl3::types::CL_BLOCKING, 0, data, &[])
                .unwrap()
        };

        (buf, load_frame_event)
    }

    pub fn copy_frame_from_buffer(
        &self,
        buffer: &opencl3::memory::Buffer<opencl3::types::cl_uchar>,
        out: &mut [u8],
        wait_events: &[opencl3::event::Event],
    ) {
        let mut events: Vec<opencl3::types::cl_event> = vec![];
        for event in wait_events.iter() {
            events.push(event.get());
        }

        let queue = self.inner.unload_queue.lock().unwrap();
        let copy_event = unsafe {
            queue
                .enqueue_read_buffer(buffer, opencl3::types::CL_BLOCKING, 0, out, &events)
                .unwrap()
        };
        copy_event.wait().unwrap();
    }

    pub fn create_video_frame_buffer(
        &self,
        num_bytes_rgba: usize,
    ) -> opencl3::memory::Buffer<opencl3::types::cl_uchar> {
        self.create_buffer(num_bytes_rgba)
    }

    pub fn create_buffer(
        &self,
        num_bytes: usize,
    ) -> opencl3::memory::Buffer<opencl3::types::cl_uchar> {
        let context = self.inner.cl_context.lock().unwrap();
        unsafe {
            opencl3::memory::Buffer::<opencl3::types::cl_uchar>::create(
                &context,
                opencl3::memory::CL_MEM_READ_WRITE,
                num_bytes,
                ptr::null_mut(),
            )
            .unwrap()
        }
    }

    fn create_image(&self, width: usize, height: usize) -> VideoBufferRef {
        let mut buffers = self.inner.video_buffers.lock().unwrap();
        let available_buffer = buffers.iter().position(|buffer| {
            buffer.available && buffer.width == width && buffer.height == height
        });
        let index = match available_buffer {
            Some(index) => {
                buffers.get_mut(index).unwrap().available = false;
                index
            }
            None => {
                let context = self.inner.cl_context.lock().unwrap();
                let buffer = unsafe {
                    opencl3::memory::Image::create(
                        &context,
                        opencl3::memory::CL_MEM_READ_WRITE,
                        &cl_image_format {
                            image_channel_order: CL_RGBA,
                            image_channel_data_type: CL_FLOAT,
                        },
                        &cl_image_desc {
                            image_type: CL_MEM_OBJECT_IMAGE2D,
                            image_width: width,
                            image_height: height,
                            image_depth: 1,
                            image_array_size: 1,
                            image_row_pitch: 0,
                            image_slice_pitch: 0,
                            num_mip_levels: 0,
                            num_samples: 0,
                            buffer: std::ptr::null_mut(),
                        },
                        std::ptr::null_mut(),
                    )
                    .unwrap()
                };
                buffers.push(VideoBuffer::new(buffer, width, height));
                buffers.len() - 1
            }
        };

        VideoBufferRef::new(self.inner.buffer_drop_event_tx.clone(), index)
    }

    // TODO: Not pub!
    pub fn create_image_from_buffer(
        &self,
        width: usize,
        height: usize,
        buffer: &opencl3::memory::Buffer<opencl3::types::cl_uchar>,
    ) -> VideoBufferRef {
        // TODO: A copy can be avoided by using cl_khr_image2d_from_buffer on platforms that support it.

        let image = self.create_image(width, height);
        let image_index = image.video_buffer_index;
        let mut buffers = self.inner.video_buffers.lock().unwrap();
        let image_buffer = buffers.get_mut(image_index).unwrap();

        let dst_origin: [usize; 3] = [0, 0, 0];
        let region: [usize; 3] = [width, height, 1];
        let queue = self.inner.process_queue.lock().unwrap();

        let wait_event = unsafe {
            queue
                .enqueue_copy_buffer_to_image(
                    buffer,
                    &mut image_buffer.buffer,
                    0,
                    dst_origin.as_ptr(),
                    region.as_ptr(),
                    &[],
                )
                .unwrap()
        };

        wait_event.wait().unwrap();

        image
    }

    pub fn create_buffer_from_image(
        &self,
        width: usize,
        height: usize,
        total_bytes: usize,
        image: phaneron_plugin::types::VideoFrame,
    ) -> opencl3::memory::Buffer<opencl3::types::cl_uchar> {
        let mut output_buffer = self.create_buffer(total_bytes);
        let buffers = self.inner.video_buffers.lock().unwrap();
        let input_buffer = buffers.get(image.buffer_index()).unwrap();
        let src_origin: [usize; 3] = [0, 0, 0];
        let region: [usize; 3] = [width, height, 1];
        let queue = self.inner.process_queue.lock().unwrap();
        let wait_event = unsafe {
            queue
                .enqueue_copy_image_to_buffer(
                    &input_buffer.buffer,
                    &mut output_buffer,
                    src_origin.as_ptr(),
                    region.as_ptr(),
                    0,
                    &[],
                )
                .unwrap()
        };
        wait_event.wait().unwrap();

        output_buffer
    }

    pub fn create_black_frame(&self, width: usize, height: usize) -> VideoFrame {
        let buffer = self.create_image(width, height);

        VideoFrame::new(VideoFrameId::default(), buffer, width, height)
    }

    pub fn create_load_shader(&self, kernel: &str) -> opencl3::kernel::Kernel {
        let context = self.inner.cl_context.lock().unwrap();
        let program = opencl3::program::Program::create_and_build_from_source(&context, kernel, "")
            .expect("Program::create_and_build_from_source failed");
        opencl3::kernel::Kernel::create(&program, "read").expect("Kernel::create failed")
    }

    pub fn create_save_shader(&self, kernel: &str) -> opencl3::kernel::Kernel {
        let context = self.inner.cl_context.lock().unwrap();
        let program = opencl3::program::Program::create_and_build_from_source(&context, kernel, "")
            .expect("Program::create_and_build_from_source failed");
        opencl3::kernel::Kernel::create(&program, "write").expect("Kernel::create failed")
    }

    pub fn create_process_shader(
        &self,
        kernel: &str,
        program_name: &str,
    ) -> phaneron_plugin::types::ProcessShader {
        let context = self.inner.cl_context.lock().unwrap();
        let program = opencl3::program::Program::create_and_build_from_source(&context, kernel, "")
            .expect("Program::create_and_build_from_source failed");
        let kernel =
            opencl3::kernel::Kernel::create(&program, program_name).expect("Kernel::create failed");

        ProcessShader_TO::from_value(
            ProcessShaderImpl::new(
                PhaneronComputeContext {
                    inner: self.inner.clone(),
                },
                kernel,
            ),
            TD_Opaque,
        )
    }

    pub fn create_loadsave_params_buffer<T>(&self, data: &[T]) -> opencl3::memory::Buffer<T> {
        let context = self.inner.cl_context.lock().unwrap();
        let mut buffer = unsafe {
            opencl3::memory::Buffer::<T>::create(
                &context,
                opencl3::memory::CL_MEM_READ_ONLY,
                data.len(),
                ptr::null_mut(),
            )
            .unwrap()
        };

        let queue = self.inner.load_queue.lock().unwrap();
        let load_buffer_event = unsafe {
            queue
                .enqueue_write_buffer(&mut buffer, opencl3::types::CL_BLOCKING, 0, data, &[])
                .unwrap()
        };
        load_buffer_event.wait().unwrap();

        buffer
    }

    pub fn run_loadsave_shader(
        &self,
        mut execute_kernel: opencl3::kernel::ExecuteKernel<'_>,
        wait_events: &[opencl3::event::Event],
    ) -> opencl3::event::Event {
        let mut events: Vec<opencl3::types::cl_event> = vec![];
        for event in wait_events.iter() {
            events.push(event.get());
        }

        execute_kernel.set_event_wait_list(&events);
        let queue = self.inner.process_queue.lock().unwrap();
        unsafe { execute_kernel.enqueue_nd_range(&queue).unwrap() }
    }

    pub fn run_process_shader(&self, mut execute_kernel: opencl3::kernel::ExecuteKernel<'_>) {
        let queue = self.inner.process_queue.lock().unwrap();
        let wait_event = unsafe { execute_kernel.enqueue_nd_range(&queue).unwrap() };

        wait_event.wait().unwrap();
    }
}

impl Clone for PhaneronComputeContext {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

struct PhaneronComputeContextInner {
    // Mutexes needed to make opencl types by treated as Send and Sync
    buffer_drop_event_tx: tokio::sync::mpsc::UnboundedSender<usize>,
    cl_context: std::sync::Mutex<opencl3::context::Context>,
    load_queue: std::sync::Mutex<opencl3::command_queue::CommandQueue>,
    process_queue: std::sync::Mutex<opencl3::command_queue::CommandQueue>,
    unload_queue: std::sync::Mutex<opencl3::command_queue::CommandQueue>,
    video_buffers: std::sync::Mutex<Vec<VideoBuffer>>,
}

#[derive(Debug)]
pub struct VideoBufferRef {
    drop_event_tx: tokio::sync::mpsc::UnboundedSender<usize>,
    video_buffer_index: usize,
}
impl VideoBufferRef {
    fn new(drop_event_tx: tokio::sync::mpsc::UnboundedSender<usize>, index: usize) -> Self {
        Self {
            drop_event_tx,
            video_buffer_index: index,
        }
    }
}

impl Drop for VideoBufferRef {
    fn drop(&mut self) {
        self.drop_event_tx.send(self.video_buffer_index).ok(); // Nothing we can do if this fails!
    }
}

struct VideoBuffer {
    available: bool,
    buffer: opencl3::memory::Image,
    width: usize,
    height: usize,
}

impl VideoBuffer {
    fn new(buffer: opencl3::memory::Image, width: usize, height: usize) -> Self {
        Self {
            available: true,
            buffer,
            width,
            height,
        }
    }
}

pub struct ProcessShaderImpl {
    context: PhaneronComputeContext,
    kernel: opencl3::kernel::Kernel,
}
impl ProcessShaderImpl {
    fn new(context: PhaneronComputeContext, kernel: opencl3::kernel::Kernel) -> Self {
        Self { context, kernel }
    }
}
impl phaneron_plugin::traits::ProcessShader for ProcessShaderImpl {
    fn run(
        &self,
        params: ShaderParams,
        global_work_size: &[usize; 2],
    ) -> RVec<phaneron_plugin::types::VideoFrame> {
        let mut output_frames: Vec<phaneron_plugin::types::VideoFrame> = vec![];
        let mut execute_kernel = opencl3::kernel::ExecuteKernel::new(&self.kernel);

        for params in params.get_params() {
            match params {
                ShaderParam::VideoFrameInput(video_frame) => {
                    let buffers = self.context.inner.video_buffers.lock().unwrap(); // TODO: Nasty reaching into context
                    let buffer = buffers.get(video_frame.buffer_index()).unwrap();
                    let image: &opencl3::memory::Image = &buffer.buffer;
                    unsafe { execute_kernel.set_arg(image) };
                }
                ShaderParam::U32Input(val) => {
                    unsafe { execute_kernel.set_arg(val) };
                }
                ShaderParam::F32Input(val) => {
                    unsafe { execute_kernel.set_arg(val) };
                }
                ShaderParam::Bool(val) => {
                    if *val {
                        unsafe {
                            execute_kernel.set_arg(&1_u8);
                        }
                    } else {
                        unsafe {
                            execute_kernel.set_arg(&0_u8);
                        }
                    }
                }
                ShaderParam::VideoFrameOutput { width, height } => {
                    let image_ref = self.context.create_image(*width, *height);
                    let image_index = image_ref.video_buffer_index;
                    let buffers = self.context.inner.video_buffers.lock().unwrap(); // TODO: Nasty!
                    let buffer = buffers.get(image_index).unwrap();
                    let image: &opencl3::memory::Image = &buffer.buffer;
                    unsafe { execute_kernel.set_arg(image) };

                    let frame =
                        VideoFrame::new(VideoFrameId::default(), image_ref, *width, *height);
                    output_frames.push(RArc::new(VideoFrame_TO::from_value(frame, TD_Opaque)))
                }
            }
        }

        execute_kernel.set_global_work_sizes(global_work_size);
        self.context.run_process_shader(execute_kernel);

        output_frames.into()
    }
}

// Safe to implement because:
// - We create execute kernels when the shader is run
// - Kernels can be safely shared
unsafe impl Send for ProcessShaderImpl {}
unsafe impl Sync for ProcessShaderImpl {}
