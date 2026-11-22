//! The project's own prepared-artifact data definitions and accessors (copied from
//! d2r-shader-iridia cache.rs and graphics.rs, same serde form). Revalidation runs in the translator.
use crate::stage_link::LinkManifest;
use crate::{Error, Result, SourceIdentity, sha256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
/// Sort every object recursively, independent of serde_json's map feature set.
/// Array order and numeric representations remain unchanged.
fn canonical_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    fn sorted(v: Value) -> Value {
        match v {
            Value::Object(map) => Value::Object(
                map.into_iter()
                    .map(|(k, v)| (k, sorted(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.into_iter().map(sorted).collect()),
            v => v,
        }
    }
    Ok(serde_json::to_vec(&sorted(serde_json::to_value(value)?))?)
}
/// Canonical metadata digest used by preparation receipts and external audits.
pub fn canonical_metadata_sha256(value: &impl Serialize) -> Result<String> {
    Ok(sha256(&canonical_bytes(value)?))
}
/// Complete converter/caller identity. Equality is necessary before replay.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CacheIdentity {
    pub schema: String,
    pub source: SourceIdentity,
    pub adapter_revision: String,
    pub dependency_lock_sha256: String,
    pub build_identity_qualification: String,
    pub consumed_build_identity_sha256: String,
    pub iridia_version: String,
    pub decoder_version: String,
    pub iridia_naga_version: String,
    pub stock_naga_version: String,
    pub program_version: u32,
    pub lifter_version: u32,
    pub precision_receipt_revision: u32,
    pub precision_policy_revision: u32,
    pub passes: BTreeMap<String, u32>,
    pub caller_contract_sha256: String,
    pub linked_interface_sha256: String,
    pub target_capabilities_sha256: String,
    pub manifest_sha256: String,
    pub payload_kind: String,
    pub payload_sha256: String,
    pub payload_bytes_sha256: String,
}
/// Owned renderer boundary. No forked Naga type or platform compiler module.
impl CacheIdentity {
    pub fn key(&self) -> Result<String> {
        Ok(sha256(&canonical_bytes(self)?))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreparedShader {
    identity: CacheIdentity,
    caller_contract: Value,
    linked_interface: Value,
    target_capabilities: Value,
    manifest: Value,
    payload: crate::payload104::ShaderPayload104,
}
impl PreparedShader {
    pub fn identity(&self) -> &CacheIdentity {
        &self.identity
    }
    pub fn payload(&self) -> &crate::payload104::ShaderPayload104 { &self.payload }
    pub fn wgsl(&self) -> Result<&str> { self.payload.wgsl() }
    pub fn wgsl_sha256(&self) -> Result<String> { Ok(sha256(self.wgsl()?.as_bytes())) }
    pub fn manifest(&self) -> &Value {
        &self.manifest
    }
    pub fn caller_contract(&self) -> &Value {
        &self.caller_contract
    }
    pub fn linked_interface(&self) -> &Value {
        &self.linked_interface
    }
    pub fn target_capabilities(&self) -> &Value {
        &self.target_capabilities
    }
    pub fn stage(&self) -> crate::Stage {
        self.identity.source.stage
    }
    pub fn entry_point(&self) -> Result<&str> {
        self.manifest["entry_point"]
            .as_str()
            .ok_or_else(|| Error::Invalid("prepared entry point".into()))
    }
    pub fn binding_layout(&self) -> Result<crate::resource_contract::BindingManifest> {
        Ok(serde_json::from_value(self.manifest["bindings"].clone())?)
    }
    pub fn resource_contract(&self) -> Result<crate::resource_contract::CallerContract> {
        Ok(serde_json::from_value(
            self.caller_contract["resources"].clone(),
        )?)
    }
    pub fn draw_transport(&self) -> Result<Option<crate::stage_link::DrawManifest>> {
        Ok(serde_json::from_value(self.manifest["draw"].clone())?)
    }
    pub fn target_contract(&self) -> Result<crate::TargetContract> {
        Ok(serde_json::from_value(self.target_capabilities.clone())?)
    }
    /// Revalidate a decoded prepared artifact; this does not attest actual GPU
    /// resource presence or execution.
    pub fn validate_artifact(&self) -> Result<()> {
        crate::rpc::validate_shader(self)
    }
}
pub const GRAPHICS_SCHEMA: &str = "d2r-iridia50-graphics-v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedGraphics {
    pub schema: String,
    pub profile: String,
    pub requires_host_profile_admission: bool,
    pub vertex: PreparedShader,
    pub fragment: PreparedShader,
    pub linkage: LinkManifest,
}
impl PreparedGraphics {
    /// Internal source/caller/WGSL consistency, not catalogue or GPU authority (runs in the translator).
    pub fn validate_artifact(&self) -> Result<()> {
        crate::rpc::validate_graphics(self)
    }
    /// The consumer passes identities obtained from its already-validated
    /// catalogue. This never manufactures that catalogue's authority.
    pub fn validate_for_profile(
        &self,
        profile: &str,
        vertex: &SourceIdentity,
        fragment: &SourceIdentity,
    ) -> Result<()> {
        if self.profile != profile
            || &self.vertex.identity().source != vertex
            || &self.fragment.identity().source != fragment
        {
            return Err(Error::Identity(
                "prepared graphics differs from consumer catalogue selection".into(),
            ));
        }
        self.validate_artifact()
    }
}
