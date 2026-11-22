//! Deterministic, narrowly supported original-shader conversion.
//!
//! Source bytecode is never bundled. CPU validation does not establish GPU parity.

pub mod compiler;
pub mod isnan;
pub mod image_coordinate_types;
pub mod lower;
pub mod reflection;
pub mod terrain;
mod terrain_compose;
mod terrain_tokens;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CONVERTER_REVISION: &str = "d2r-shader192-buffer-draw-int-tri-actor-null-v3";
pub const NAGA_VERSION: &str = "30.0.1";

#[derive(Debug, Error)]
pub enum ShaderError {
    #[error("invalid shader: {0}")]
    Invalid(String),
    #[error("unsupported shader construct: {0}")]
    Unsupported(String),
    #[error("shader validation: {0}")]
    Validation(String),
    #[error("shader I/O: {0}")]
    Io(#[from] std::io::Error),
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BufferBinding {
    pub group: u32,
    pub binding: u32,
    pub original_register: String,
    /// A declared source resource-view contract, not inferred from Unknown format.
    pub format: String,
}

