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

use std::fmt::{Debug, Display};

use abi_stable::std_types::RVec;

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
    pub audio_buffers: RVec<RVec<f32>>,
}

impl AudioFrame {
    pub fn new(id: AudioFrameId, audio_buffers: Vec<Vec<f32>>) -> Self {
        let mut buffers = RVec::with_capacity(audio_buffers.len());
        for buffer in audio_buffers {
            buffers.push(buffer.into());
        }
        Self {
            id,
            audio_buffers: buffers,
        }
    }
}

impl phaneron_plugin::traits::AudioFrame for AudioFrame {
    fn buffers(&self) -> &RVec<RVec<f32>> {
        &self.audio_buffers
    }
}
