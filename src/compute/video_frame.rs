use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoBufferId(String);
impl VideoBufferId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for VideoBufferId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for VideoBufferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoFrameId(String);
impl VideoFrameId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for VideoFrameId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for VideoFrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub id: VideoFrameId,
    video_buffer: Arc<opencl3::memory::Image>,
    width: usize,
    height: usize,
}

impl VideoFrame {
    pub fn new(
        id: VideoFrameId,
        video_buffer: opencl3::memory::Image,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            id,
            video_buffer: Arc::new(video_buffer),
            width,
            height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn get_buffer(&self) -> &Arc<opencl3::memory::Image> {
        &self.video_buffer
    }
}

// Safe to implement because:
// - opencl buffers can be sent between threads
// - we only ever allow immutable access to the buffer
unsafe impl Send for VideoFrame {}
unsafe impl Sync for VideoFrame {}
