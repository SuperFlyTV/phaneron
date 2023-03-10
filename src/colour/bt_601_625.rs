use super::ColourSpec;

pub const COLOUR_SPEC_BT_601_625: ColourSpec = ColourSpec {
    // https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.601-7-201103-I!!PDF-E.pdf
    kR: 0.299,
    kB: 0.114,
    rx: 0.64,
    ry: 0.33,
    gx: 0.29,
    gy: 0.6,
    bx: 0.15,
    by: 0.06,
    wx: 0.3127,
    wy: 0.329,
    alpha: 1.099,
    beta: 0.018,
    gamma: 0.45,
    delta: 4.5,
};
