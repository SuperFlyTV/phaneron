use std::{sync::Mutex, time::Instant};

use abi_stable::{
    sabi_trait::TD_Opaque,
    std_types::{ROption, RString},
};
use log::info;

use phaneron_plugin::{
    traits::Node_TO, types::FromRGBA, types::Node, types::NodeContext, types::ProcessFrameContext,
    ColourSpace, InterlaceMode, VideoFormat, VideoInputId,
};

pub struct TurboConsumerHandle {
    node_id: String,
}
impl TurboConsumerHandle {
    pub(super) fn new(node_id: String) -> Self {
        Self { node_id }
    }
}
impl phaneron_plugin::traits::NodeHandle for TurboConsumerHandle {
    fn initialize(&self, context: NodeContext, _configuration: ROption<RString>) -> Node {
        let node = TurboConsumer::new(self.node_id.clone(), context);

        Node_TO::from_value(node, TD_Opaque)
    }
}

pub struct TurboConsumer {
    node_id: String,
    context: NodeContext,
    total: Mutex<u128>,
    frames: Mutex<u128>,
    from_rgba: Mutex<Option<FromRGBA>>,
    last_frame_time: Mutex<Option<Instant>>,
    input_id: VideoInputId,
}

impl TurboConsumer {
    pub fn new(node_id: String, context: NodeContext) -> Self {
        let input_id = context.add_video_input();

        Self {
            node_id,
            context,
            total: Default::default(),
            frames: Default::default(),
            from_rgba: Default::default(),
            last_frame_time: Default::default(),
            input_id,
        }
    }
}

impl phaneron_plugin::traits::Node for TurboConsumer {
    fn apply_state(&self, state: RString) -> bool {
        false
    }

    fn process_frame(&self, frame_context: ProcessFrameContext) {
        let mut from_rgba_lock = self.from_rgba.lock().unwrap();
        let from_rgba = from_rgba_lock.get_or_insert(self.context.create_from_rgba(
            &VideoFormat::YUV420p,
            &ColourSpace::sRGB.colour_spec(),
            1920,
            1080,
            InterlaceMode::Progressive,
        ));
        let mut last_frame_time = self.last_frame_time.lock().unwrap();
        let frame = frame_context
            .get_video_input(&self.input_id)
            .unwrap_or(frame_context.get_black_frame())
            .clone();
        let frame = from_rgba.process_frame(&frame_context, frame.frame);

        let copy_context = frame_context.submit().unwrap();
        let _frame = from_rgba.copy_frame(&copy_context, frame);

        let timer = last_frame_time.replace(Instant::now());
        if let Some(timer) = timer {
            let elapsed = timer.elapsed();
            let mut total = self.total.lock().unwrap();
            let mut frames = self.frames.lock().unwrap();
            *total += elapsed.as_micros();
            *frames += 1;
            let avg = *total / *frames;
            let avg_ms = avg as f32 / 1000.0;
            let fps = f32::floor(1000.0 / avg_ms);
            info!(
                "Average frame time: {}us ({}ms) (~{}fps)",
                avg,
                f32::floor(avg_ms),
                fps
            );
        }
    }
}
