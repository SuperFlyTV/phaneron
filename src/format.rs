use crate::io::{InterlaceMode, Packer, Unpacker};

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

pub enum VideoFormat {
    BRGA8,
    RGBA8,
    V210,
    YUV420p,
    YUV422p8,
    YUV422p10,
}

impl VideoFormat {
    pub fn get_reader(&self, width: usize, height: usize) -> Box<dyn Packer> {
        match self {
            VideoFormat::BRGA8 => Box::new(BGRA8Reader::new(width, height)),
            VideoFormat::RGBA8 => Box::new(RGBA8Reader::new(width, height)),
            VideoFormat::V210 => Box::new(V210Reader::new(width, height)),
            VideoFormat::YUV420p => Box::new(YUV420pReader::new(width, height)),
            VideoFormat::YUV422p8 => Box::new(YUV422p8Reader::new(width, height)),
            VideoFormat::YUV422p10 => Box::new(YUV422p10Reader::new(width, height)),
        }
    }

    pub fn get_writer(
        &self,
        width: usize,
        height: usize,
        interlace: InterlaceMode,
    ) -> Box<dyn Unpacker> {
        match self {
            VideoFormat::BRGA8 => Box::new(BGRA8Writer::new(width, height, interlace)),
            VideoFormat::RGBA8 => Box::new(RGBA8Writer::new(width, height, interlace)),
            VideoFormat::V210 => Box::new(V210Writer::new(width, height, interlace)),
            VideoFormat::YUV420p => Box::new(YUV420pWriter::new(width, height, interlace)),
            VideoFormat::YUV422p8 => Box::new(YUV422p8Writer::new(width, height, interlace)),
            VideoFormat::YUV422p10 => Box::new(YUV422p10Writer::new(width, height, interlace)),
        }
    }
}
