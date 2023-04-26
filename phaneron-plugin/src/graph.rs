use std::fmt::Display;

use abi_stable::{std_types::RString, StableAbi};
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, StableAbi)]
pub struct AudioInputId(RString);
impl AudioInputId {
    pub fn new_from(id: RString) -> Self {
        Self(id)
    }
}
impl Default for AudioInputId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string().into())
    }
}
impl Display for AudioInputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, StableAbi)]
pub struct AudioOutputId(RString);
impl AudioOutputId {
    pub fn new_from(id: RString) -> Self {
        Self(id)
    }
}
impl Default for AudioOutputId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string().into())
    }
}
impl Display for AudioOutputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, StableAbi)]
pub struct VideoInputId(RString);
impl VideoInputId {
    pub fn new_from(id: RString) -> Self {
        Self(id)
    }
}
impl Default for VideoInputId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string().into())
    }
}
impl Display for VideoInputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, StableAbi)]
pub struct VideoOutputId(RString);
impl VideoOutputId {
    pub fn new_from(id: RString) -> Self {
        Self(id)
    }
}
impl Default for VideoOutputId {
    fn default() -> Self {
        Self(uuid::Uuid::new_v4().to_string().into())
    }
}
impl Display for VideoOutputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
