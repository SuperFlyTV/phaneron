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

use phaneron_plugin::InterlaceMode;

use crate::io::{Packer, Unpacker};

use self::{
    bgra::{BGRA8Reader, BGRA8Writer},
    rgba8::{RGBA8Reader, RGBA8Writer},
    v210::{V210Reader, V210Writer},
    yuv420p::{YUV420pReader, YUV420pWriter},
    yuv422p10::{YUV422p10Reader, YUV422p10Writer},
    yuv422p8::{YUV422p8Reader, YUV422p8Writer},
};

pub mod bgra;
pub mod rgba8;
pub mod v210;
pub mod yuv420p;
pub mod yuv422p10;
pub mod yuv422p8;

pub trait VideoFormat {
    fn get_reader(&self, width: usize, height: usize) -> Box<dyn Packer>;
    fn get_writer(
        &self,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> Box<dyn Unpacker>;
}

impl VideoFormat for phaneron_plugin::VideoFormat {
    fn get_reader(&self, width: usize, height: usize) -> Box<dyn Packer> {
        match self {
            phaneron_plugin::VideoFormat::BGRA8 => Box::new(BGRA8Reader::new(width, height)),
            phaneron_plugin::VideoFormat::RGBA8 => Box::new(RGBA8Reader::new(width, height)),
            phaneron_plugin::VideoFormat::V210 => Box::new(V210Reader::new(width, height)),
            phaneron_plugin::VideoFormat::YUV420p => Box::new(YUV420pReader::new(width, height)),
            phaneron_plugin::VideoFormat::YUV422p8 => Box::new(YUV422p8Reader::new(width, height)),
            phaneron_plugin::VideoFormat::YUV422p10 => {
                Box::new(YUV422p10Reader::new(width, height))
            }
        }
    }

    fn get_writer(
        &self,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> Box<dyn Unpacker> {
        match self {
            phaneron_plugin::VideoFormat::BGRA8 => {
                Box::new(BGRA8Writer::new(width, height, interlace))
            }
            phaneron_plugin::VideoFormat::RGBA8 => {
                Box::new(RGBA8Writer::new(width, height, interlace))
            }
            phaneron_plugin::VideoFormat::V210 => {
                Box::new(V210Writer::new(width, height, interlace))
            }
            phaneron_plugin::VideoFormat::YUV420p => {
                Box::new(YUV420pWriter::new(width, height, interlace))
            }
            phaneron_plugin::VideoFormat::YUV422p8 => {
                Box::new(YUV422p8Writer::new(width, height, interlace))
            }
            phaneron_plugin::VideoFormat::YUV422p10 => {
                Box::new(YUV422p10Writer::new(width, height, interlace))
            }
        }
    }
}
