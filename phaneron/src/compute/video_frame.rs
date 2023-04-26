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

use std::{
    fmt::{Debug, Display},
    sync::Arc,
};

use super::VideoBufferRef;

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
    video_buffer_ref: Arc<VideoBufferRef>,
    width: usize,
    height: usize,
}

impl VideoFrame {
    pub fn new(
        id: VideoFrameId,
        video_buffer_ref: VideoBufferRef,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            id,
            video_buffer_ref: Arc::new(video_buffer_ref),
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
}

impl phaneron_plugin::traits::VideoFrame for VideoFrame {
    fn buffer_index(&self) -> usize {
        self.video_buffer_ref.video_buffer_index
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }
}
