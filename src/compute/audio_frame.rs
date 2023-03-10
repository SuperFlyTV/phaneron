use std::fmt::{Debug, Display};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioBufferId(String);
impl AudioBufferId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for AudioBufferId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for AudioBufferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioFrameId(String);
impl AudioFrameId {
    pub fn new_from(id: String) -> Self {
        Self(id)
    }
}
impl Default for AudioFrameId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Display for AudioFrameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub id: AudioFrameId,
    pub audio_buffers: Vec<Vec<f32>>,
}

impl AudioFrame {
    pub fn new(id: AudioFrameId, audio_buffers: Vec<Vec<f32>>) -> Self {
        Self { id, audio_buffers }
    }
}
