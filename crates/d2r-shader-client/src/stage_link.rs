//! The project's own draw/link data definitions (copied from d2r-shader-iridia
//! stage_link.rs, same serde form; linking runs in the translator).
use serde::{Deserialize, Serialize};
pub const LINK_REVISION: &str = "d2r-iridia-no-padding-semantic-link52-v1";
pub const DRAW_REVISION: &str = "d2r-iridia-explicit-first-instance52-v1";
/// Primitive topologies a linked pair may be drawn with. Topology is pipeline state, not shader
/// state: the vertex/pixel modules, their semantic linkage and the first-instance transport are the
/// same for every admitted topology, so no shader arithmetic depends on the choice. A triangle strip
/// is admitted for non-indexed and indexed draws alike: the stages see only the vertex/instance IDs
/// (in D3D12 and WebGPU the vertex or index value plus the base vertex), while the strip index
/// format and restart value are host pipeline state outside the modules. The recorded topology is
/// part of the link manifest; the host must create its pipeline with exactly that topology.
pub const TOPOLOGIES: [&str; 2] = ["triangle-list", "triangle-strip"];
pub fn supported_topology(topology: &str) -> bool {
    TOPOLOGIES.contains(&topology)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawContract {
    pub revision: String,
    pub topology: String,
    pub base_group: u32,
    pub base_binding: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawManifest {
    pub revision: String,
    pub group: u32,
    pub binding: u32,
    pub uniform_bytes: u32,
    pub first_instance_offset: u32,
    pub source_body_unchanged: bool,
    pub normalization: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedVarying {
    pub semantic: String,
    pub index: u32,
    pub scalar: String,
    pub lanes: u8,
    pub producer_register: u32,
    pub consumer_register: u32,
    pub required_mask: u8,
    pub interpolation: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkManifest {
    pub revision: String,
    pub topology: String,
    pub consumed: Vec<LinkedVarying>,
    pub padding_allowed: bool,
    pub emitted_notes: Vec<String>,
}
