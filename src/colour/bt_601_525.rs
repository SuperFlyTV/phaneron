use super::ColourSpec;

pub const COLOUR_SPEC_BT_601_525: ColourSpec = ColourSpec {
    // https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.601-7-201103-I!!PDF-E.pdf
    kR: 0.299,
    kB: 0.114,
    rx: 0.63,
    ry: 0.34,
    gx: 0.31,
    gy: 0.595,
    bx: 0.155,
    by: 0.07,
    wx: 0.3127,
    wy: 0.329,
    alpha: 1.099,
    beta: 0.018,
    gamma: 0.45,
    delta: 4.5,
};
