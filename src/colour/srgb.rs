use super::ColourSpec;

pub const COLOUR_SPEC_SRGB: ColourSpec = ColourSpec {
    // https://en.wikipedia.org/wiki/SRGB
    kR: 0.2126, // BT.709
    kB: 0.0722, // BT.709
    rx: 0.64,
    ry: 0.33,
    gx: 0.3,
    gy: 0.6,
    bx: 0.15,
    by: 0.06,
    wx: 0.3127,
    wy: 0.329,
    alpha: 1.055,
    beta: 0.0031308,
    gamma: 1.0 / 2.4,
    delta: 12.92,
};
