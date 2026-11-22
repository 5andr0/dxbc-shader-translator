//! The project's own resource-contract data definitions (copied from d2r-shader-iridia
//! resource_contract.rs, same serde form; the contract application runs in the translator).
use serde::{Deserialize, Serialize};
pub const RESOURCE_REVISION: &str = "d2r-iridia-explicit-resources52-v2";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterClass {
    ConstantBuffer,
    ShaderResource,
    Sampler,
    UnorderedAccess,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BufferView {
    pub source_first_element: u64,
    pub source_num_elements: u64,
    pub source_stride_bytes: u64,
    pub source_resource_bytes: u64,
}
impl BufferView {
    /// Exact standalone range the host must bind, without padding or expansion.
    pub fn range(&self) -> Result<std::ops::Range<u64>, String> {
        if self.source_stride_bytes == 0 || self.source_num_elements == 0 {
            return Err("zero buffer stride/count".into());
        }
        let start = self
            .source_first_element
            .checked_mul(self.source_stride_bytes)
            .ok_or("view base overflow")?;
        let len = self
            .source_num_elements
            .checked_mul(self.source_stride_bytes)
            .ok_or("view length overflow")?;
        let end = start.checked_add(len).ok_or("view end overflow")?;
        if end > self.source_resource_bytes || len > u32::MAX as u64 {
            return Err("buffer view exceeds source/bounded shader indexing".into());
        }
        Ok(start..end)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceContractKind {
    ConstantBuffer {
        bound_bytes: u32,
    },
    RawBuffer {
        view: BufferView,
    },
    StructuredBuffer {
        stride_bytes: u32,
        view: BufferView,
    },
    /// Only a full four-lane 32-bit format is admitted in this first pass.
    /// Narrow/normalized formats need an explicit IR conversion, never padding.
    TypedBuffer {
        format: String,
        view: BufferView,
    },
    Texture {
        dimension: String,
        arrayed: bool,
        multisampled: bool,
        sample: String,
    },
    Sampler {
        comparison: bool,
    },
    StorageBuffer {
        access: String,
        stride_bytes: u32,
        view: BufferView,
    },
    StorageImage {
        dimension: String,
        arrayed: bool,
        access: String,
        format: String,
    },
    /// A retained original declaration, proved entrypoint-unread after a
    /// separately admitted source/caller projection. This is not a texture view.
    UnreadDeclaration {
        original_resource_sha256: String,
        reason: UnreadReason,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnreadReason {
    MatchedNull,
    UnobservedSourceBranch,
    UnusedProjectionDependency,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceContract {
    pub class: RegisterClass,
    pub register: u32,
    pub space: u32,
    pub count: u32,
    pub group: u32,
    pub binding: u32,
    pub resource: ResourceContractKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallerContract {
    pub revision: String,
    pub resources: Vec<ResourceContract>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingManifestEntry {
    pub source_resource_id: u32,
    pub class: RegisterClass,
    pub register: u32,
    pub space: u32,
    pub group: u32,
    pub binding: u32,
    pub resource: ResourceContractKind,
    pub physical_binding_required: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingManifest {
    pub revision: String,
    pub resources: Vec<BindingManifestEntry>,
    pub standalone_exact_view_rebase_required: bool,
    pub host_resource_presence_proved: bool,
    pub resolved_notes: Vec<ResolvedNote>,
    pub storage_formats: Vec<StorageFormatChange>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageFormatChange {
    pub resource_id: u32,
    pub original_ir_format: String,
    pub actual_view_format: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedNote {
    pub original_note: String,
    pub rule: String,
    pub source_resource_ids: Vec<u32>,
    pub source_and_transport_bits: String,
}
