use abi_stable::StableAbi;

/// Supported audio I/O formats.
/// Audio will be converted to 32 bit floating-point on input.
#[repr(C)]
#[derive(StableAbi)]
pub enum AudioFormat {
    I16,
    U16,
    F32,
    I32,
}

/// Supported audio channel layouts.
#[repr(C)]
#[derive(StableAbi)]
#[allow(non_camel_case_types)]
pub enum AudioChannelLayout {
    Mono,
    L,
    R,
    L_R,
    R_L,
}
