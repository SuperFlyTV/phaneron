use super::ColourSpec;

/// Colour space transformation for BT.709.
/// Reference: <https://www.itu.int/dms_pubrec/itu-r/rec/bt/R-REC-BT.709-6-201506-I!!PDF-E.pdf>
pub const COLOUR_SPEC_BT_709: ColourSpec = ColourSpec {
    kR: 0.2126,
    kB: 0.0722,
    rx: 0.64,
    ry: 0.33,
    gx: 0.3,
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
