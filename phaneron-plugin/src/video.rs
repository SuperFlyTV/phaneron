use abi_stable::StableAbi;

/// Supported interlacing modes.
#[repr(C)]
#[derive(StableAbi)]
pub enum InterlaceMode {
    Progressive,
    TopField,
    BottomField,
}

/// Supported pixel packing formats.
#[repr(C)]
#[derive(StableAbi)]
pub enum VideoFormat {
    BGRA8,
    RGBA8,
    V210,
    YUV420p,
    YUV422p8,
    YUV422p10,
}
