//! Target contract data definitions (copied from d2r-shader-iridia prepare.rs).
use serde::{Deserialize, Serialize};
pub const PAIR_SCHEMA:&str="d2r-iridia50-linked-pair-v1";
pub const ICE_PROFILE:&str="ice-bolt-original-mesh-snapshot-profile1";
/// Explicit limits of the eventual target. This input is part of the cache
/// identity; only the consumer/device can prove its reported limits are actual.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetContract {
    pub revision:String,
    pub backend:String,
    pub max_bind_groups:u32,
    pub max_bindings_per_bind_group:u32,
    pub max_uniform_buffer_binding_size:u64,
    pub max_storage_buffer_binding_size:u64,
    #[serde(default)]
    pub compute:Option<ComputeLimits>,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeLimits {pub max_invocations_per_workgroup:u32,pub max_workgroup_size:[u32;3],pub max_workgroup_storage_size:u64}
