use super::ColourSpec;

pub const COLOUR_SPEC_BT_2020: ColourSpec = ColourSpec {
    // https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.2020-2-201510-I!!PDF-E.pdf
    kR: 0.2627,
    kB: 0.0593,
    rx: 0.708,
    ry: 0.292,
    gx: 0.170,
    gy: 0.797,
    bx: 0.131,
    by: 0.046,
    wx: 0.3127,
    wy: 0.3290,
    alpha: 1.099,
    beta: 0.018,
    gamma: 0.45,
    delta: 4.5,
};
