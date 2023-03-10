#![allow(non_snake_case)]

use nalgebra::{Matrix3, Matrix3x1, Matrix3x4, Matrix4x3};

pub use self::{
    bt_2020::COLOUR_SPEC_BT_2020, bt_601_525::COLOUR_SPEC_BT_601_525,
    bt_601_625::COLOUR_SPEC_BT_601_625, bt_709::COLOUR_SPEC_BT_709, srgb::COLOUR_SPEC_SRGB,
};

mod bt_2020;
mod bt_601_525;
mod bt_601_625;
mod bt_709;
mod srgb;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

const LUT_ARRAY_ENTRIES: usize = 65536;
pub fn gamma_to_linear_lut(colour_spec: &ColourSpec) -> Vec<f32> {
    let mut lut_array = vec![1.0; LUT_ARRAY_ENTRIES];

    let alpha = colour_spec.alpha;
    let delta = colour_spec.delta;
    let beta = colour_spec.beta;
    let gamma = colour_spec.gamma;

    for (i, entry) in lut_array.iter_mut().enumerate() {
        let fi = (i as f32) / ((LUT_ARRAY_ENTRIES - 1) as f32);
        if fi < beta {
            *entry = fi / delta;
        } else {
            *entry = f32::powf((fi + (alpha - 1.0)) / alpha, 1.0 / gamma);
        }
    }

    lut_array
}

pub fn linear_to_gamma_lut(colour_spec: &ColourSpec) -> Vec<f32> {
    let mut lut_array = vec![1.0; LUT_ARRAY_ENTRIES];

    let alpha = colour_spec.alpha;
    let beta = colour_spec.beta;
    let gamma = colour_spec.gamma;
    let delta = colour_spec.delta;

    for (i, entry) in lut_array.iter_mut().enumerate() {
        let fi = (i as f32) / ((LUT_ARRAY_ENTRIES - 1) as f32);
        if fi < beta {
            *entry = fi * delta;
        } else {
            *entry = alpha * f32::powf(fi, gamma) - (alpha - 1.0);
        }
    }

    lut_array
}

pub fn rgb_to_xyz_matrix(colour_spec: &ColourSpec) -> Matrix3<f32> {
    let w = Matrix3x1::new(
        colour_spec.wx,
        colour_spec.wy,
        1.0 - colour_spec.wx - colour_spec.wy,
    );
    let w = w * (1.0 / w.data.0[0][1]);

    let xyz = Matrix3::new(
        colour_spec.rx,
        colour_spec.gx,
        colour_spec.bx,
        colour_spec.ry,
        colour_spec.gy,
        colour_spec.by,
        1.0 - colour_spec.rx - colour_spec.ry,
        1.0 - colour_spec.gx - colour_spec.gy,
        1.0 - colour_spec.bx - colour_spec.by,
    );
    let xyz_invert = xyz.try_inverse().unwrap();
    let xyz_scale_factors = xyz_invert * w;

    let xyz_scale = Matrix3::new(
        xyz_scale_factors.data.0[0][0],
        0.0,
        0.0,
        0.0,
        xyz_scale_factors.data.0[0][1],
        0.0,
        0.0,
        0.0,
        xyz_scale_factors.data.0[0][2],
    );

    xyz * xyz_scale
}

pub fn xyz_to_rgb_matrix(colour_spec: &ColourSpec) -> Matrix3<f32> {
    rgb_to_xyz_matrix(colour_spec).try_inverse().unwrap()
}

pub fn rgb_to_common_space_matrix(source_colour_spec: &ColourSpec) -> Matrix3<f32> {
    (xyz_to_rgb_matrix(&COLOUR_SPEC_BT_709) * rgb_to_xyz_matrix(source_colour_spec)).transpose()
}

pub fn common_space_to_rgb_matrix(destination_colour_space: &ColourSpec) -> Matrix3<f32> {
    (xyz_to_rgb_matrix(destination_colour_space) * rgb_to_xyz_matrix(&COLOUR_SPEC_BT_709))
        .transpose()
}

pub fn ycbcr_to_rgb_matrix(
    colour_spec: &ColourSpec,
    number_of_bits: usize,
    luma_black: f32,
    luma_white: f32,
    chroma_range: f32,
) -> Matrix4x3<f32> {
    let chroma_null = (128u32 << (number_of_bits - 8)) as f32;
    let luma_range = luma_white - luma_black;

    let kR = colour_spec.kR;
    let kB = colour_spec.kB;
    let kG = 1.0 - kR - kB;

    let Yr = 1.0;
    let Ur = 0.0;
    let Vr = 1.0 - kR;

    let Yg = 1.0;
    let Ug = (-(1.0 - kB) * kB) / kG;
    let Vg = (-(1.0 - kR) * kR) / kG;

    let Yb = 1.0;
    let Ub = 1.0 - kB;
    let Vb = 0.0;

    let colour_matrix = Matrix3::new(Yr, Ur, Vr, Yg, Ug, Vg, Yb, Ub, Vb);

    let Yy = 1.0 / luma_range;
    let Uy = 0.0;
    let Vy = 0.0;
    let Oy = -luma_black / luma_range;

    let Yu = 0.0;
    let Uu = (1.0 / chroma_range) * 2.0;
    let Vu = 0.0;
    let Ou = -(chroma_null / chroma_range) * 2.0;

    let Yv = 0.0;
    let Uv = 0.0;
    let Vv = (1.0 / chroma_range) * 2.0;
    let Ov = -(chroma_null / chroma_range) * 2.0;

    let scale_matrix = Matrix3x4::new(Yy, Uy, Vy, Oy, Yu, Uu, Vu, Ou, Yv, Uv, Vv, Ov);

    (colour_matrix * scale_matrix).transpose()
}

pub fn rgb_to_ycbcr_matrix(
    colour_spec: &ColourSpec,
    number_of_bits: usize,
    luma_black: f32,
    luma_white: f32,
    chroma_range: f32,
) -> Matrix4x3<f32> {
    let chroma_null = (128u32 << (number_of_bits - 8)) as f32;
    let luma_range = luma_white - luma_black;
    let kR = colour_spec.kR;
    let kB = colour_spec.kB;
    let kG = 1.0 - kR - kB;

    let Yy = luma_range;
    let Uy = 0.0;
    let Vy = 0.0;

    let Yu = 0.0;
    let Uu = chroma_range / 2.0;
    let Vu = 0.0;

    let Yv = 0.0;
    let Uv = 0.0;
    let Vv = chroma_range / 2.0;

    let scale_matrix = Matrix3::new(Yy, Uy, Vy, Yu, Uu, Vu, Yv, Uv, Vv);

    let Ry = kR;
    let Gy = kG;
    let By = kB;
    let Oy = luma_black / luma_range;

    let Ru = -kR / (1.0 - kB);
    let Gu = -kG / (1.0 - kB);
    #[allow(clippy::eq_op)]
    let Bu = (1.0 - kB) / (1.0 - kB);
    let Ou = (chroma_null / chroma_range) * 2.0;

    #[allow(clippy::eq_op)]
    let Rv = (1.0 - kR) / (1.0 - kR);
    let Gv = -kG / (1.0 - kR);
    let Bv = -kB / (1.0 - kR);
    let Ov = (chroma_null / chroma_range) * 2.0;

    let colour_matrix = Matrix3x4::new(Ry, Gy, By, Oy, Ru, Gu, Bu, Ou, Rv, Gv, Bv, Ov);

    (scale_matrix * colour_matrix).transpose()
}
