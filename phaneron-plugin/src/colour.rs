//! This module contains colour space transformation definitions.
//! Some built-in colour spaces are provided, but custom spaces may be
//! defined using the [`ColourSpec`] struct.

use abi_stable::StableAbi;

pub use self::{
    bt_2020::COLOUR_SPEC_BT_2020, bt_601_525::COLOUR_SPEC_BT_601_525,
    bt_601_625::COLOUR_SPEC_BT_601_625, bt_709::COLOUR_SPEC_BT_709, srgb::COLOUR_SPEC_SRGB,
};

mod bt_2020;
mod bt_601_525;
mod bt_601_625;
mod bt_709;
mod srgb;

/// Built-in colour space definitions.
#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, StableAbi)]
pub enum ColourSpace {
    #[allow(non_camel_case_types)]
    sRGB,
    #[allow(non_camel_case_types)]
    BT_601_625,
    #[allow(non_camel_case_types)]
    BT_601_525,
    #[allow(non_camel_case_types)]
    BT_709,
    #[allow(non_camel_case_types)]
    BT_2020,
}

/// Defines the transformation function for a colourspace.
/// May be used to define custom colour spaces.
#[repr(C)]
#[derive(StableAbi)]
#[allow(non_snake_case)]
pub struct ColourSpec {
    pub kR: f32,
    pub kB: f32,
    pub rx: f32,
    pub ry: f32,
    pub gx: f32,
    pub gy: f32,
    pub bx: f32,
    pub by: f32,
    pub wx: f32,
    pub wy: f32,
    pub alpha: f32,
    pub beta: f32,
    pub gamma: f32,
    pub delta: f32,
}

impl ColourSpace {
    pub fn colour_spec(&self) -> ColourSpec {
        match self {
            ColourSpace::BT_2020 => COLOUR_SPEC_BT_2020,
            ColourSpace::BT_601_525 => COLOUR_SPEC_BT_601_525,
            ColourSpace::BT_601_625 => COLOUR_SPEC_BT_601_625,
            ColourSpace::BT_709 => COLOUR_SPEC_BT_709,
            ColourSpace::sRGB => COLOUR_SPEC_SRGB,
        }
    }
}
