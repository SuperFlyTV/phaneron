use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use tokio::{
    sync::{mpsc::UnboundedSender, Mutex},
    time::Instant,
};
use tracing::info;

use crate::{
    colour::ColourSpace,
    compute::{audio_frame::AudioFrame, video_frame::VideoFrame},
    format::VideoFormat,
    graph::{AudioInputId, AudioOutputId, NodeId, VideoInputId, VideoOutputId},
    io::{FromRGBA, InterlaceMode},
    node_context::{Node, NodeContext, ProcessFrameContext},
};

pub struct TurboConsumerPlugin {}

impl TurboConsumerPlugin {
    pub fn load() -> Self {
        Self {}
    }

    pub async fn initialize(&mut self) {
        info!("Turbo Consumer plugin initializing");
        info!("Turbo Consumer plugin initialized");
    }

    pub async fn create_node(&mut self, node_id: NodeId) -> TurboConsumer {
        TurboConsumer::new(node_id)
    }
}

pub struct TurboConsumer {
    node_id: NodeId,
    from_rgba: Mutex<Option<FromRGBA>>,
    context: Mutex<Option<NodeContext>>,
    last_frame_time: Mutex<Option<Instant>>,
    sender: Mutex<Option<UnboundedSender<Duration>>>,
}

impl TurboConsumer {
    pub fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            from_rgba: Default::default(),
            context: Default::default(),
            last_frame_time: Default::default(),
            sender: Default::default(),
        }
    }

    pub async fn initialize(&mut self, context: NodeContext) {
        info!("Turbo Consumer {} initializing", self.node_id);

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<Duration>();

        tokio::spawn(async move {
            let mut total: u128 = 0;
            let mut frames: u128 = 0;
            loop {
                while let Some(elapsed) = receiver.recv().await {
                    total += elapsed.as_micros();
                    frames += 1;
                    let avg = total / frames;
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
        });

        context
            .add_video_input(VideoInputId::new_from("turbo_consumer_input".to_string()))
            .await
            .unwrap();

        self.context.lock().await.replace(context);
        self.sender.lock().await.replace(sender);

        info!("Turbo Consumer {} initialized", self.node_id);
    }
}

#[async_trait]
impl Node for TurboConsumer {
    async fn apply_state(&self, state: String) -> bool {
        false
    }
    async fn process_frame(
        &self,
        frame_context: ProcessFrameContext,
        video_frames: HashMap<VideoInputId, (VideoOutputId, VideoFrame)>,
        audio_frames: HashMap<AudioInputId, (AudioOutputId, AudioFrame)>,
        black_frame: (VideoOutputId, VideoFrame),
        silence_frame: (AudioOutputId, AudioFrame),
    ) {
        let mut from_rgba_lock = self.from_rgba.lock().await;
        let from_rgba = from_rgba_lock.get_or_insert(
            self.context
                .lock()
                .await
                .as_mut()
                .unwrap()
                .create_from_rgba(
                    &VideoFormat::YUV420p,
                    &ColourSpace::sRGB,
                    1920,
                    1080,
                    InterlaceMode::Progressive,
                ),
        );
        let mut last_frame_time = self.last_frame_time.lock().await;
        let (_pipe_id, frame) = video_frames.values().next().unwrap_or(&black_frame).clone();
        let frame = from_rgba.process_frame(frame).await;

        let copy_context = frame_context.submit().await;
        let _frame = from_rgba.copy_frame(&copy_context, frame).await;

        let timer = last_frame_time.replace(Instant::now());
        if let Some(timer) = timer {
            let elapsed = timer.elapsed();
            let sender_lock = self.sender.lock().await;
            let sender = sender_lock.as_ref().unwrap();
            sender.send(elapsed).unwrap();
        }
    }
}
