use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

/// Game-version tolerance (`D2R_VERSION_TOLERANT=1`): when an original
/// shader resource differs from the pinned one (another D2R patch), find the pinned program
/// container anywhere in the installed resource; failing that, keep the container at the
/// pinned offset (the installed program). Every fallback prints a `VERSION WARN` line.
pub fn tolerant_dxbc<'a>(path: &str, resource: &'a [u8], offset: usize, byte_count: usize, want: &str) -> Option<&'a [u8]> {
    if std::env::var_os("D2R_VERSION_TOLERANT").is_none_or(|v| v.is_empty() || v == "0") {
        return None;
    }
    let container = |at: usize| -> Option<&'a [u8]> {
        let head = resource.get(at..at + 32)?;
        if &head[..4] != b"DXBC" { return None; }
        let n = u32::from_le_bytes(head[24..28].try_into().ok()?) as usize;
        resource.get(at..at.checked_add(n)?)
    };
    let mut at = 0;
    while let Some(i) = resource.get(at..).and_then(|r| r.windows(4).position(|w| w == b"DXBC")) {
        if let Some(c) = container(at + i).filter(|c| sha256(c) == want) {
            eprintln!("VERSION WARN: {path}: installed resource differs from the pinned one; pinned program {} found at offset {} (pinned offset {offset})", &want[..12.min(want.len())], at + i);
            return Some(c);
        }
        at += i + 4;
    }
    let c = container(offset).or_else(|| resource.get(offset..offset.checked_add(byte_count)?).filter(|c| c.starts_with(b"DXBC")))?;
    eprintln!("VERSION WARN: {path}: pinned program {} not in the installed resource; using the installed program at offset {offset} ({} bytes) - its output may differ", &want[..12.min(want.len())], c.len());
    Some(c)
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum Stage { Vertex, Fragment, Compute, Hull, Domain }

/// Both resource and original complete DXBC-container identities are mandatory.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    pub path: String,
    pub resource_sha256: String,
    pub resource_bytes: u64,
    pub offset: u64,
    pub byte_count: u64,
    pub dxbc_sha256: String,
    pub stage: Stage,
}
impl SourceIdentity {
    pub fn validate(&self) -> Result<()> {
        if self.path.is_empty() || self.path.len() > 1024 || self.path.starts_with('/')
            || self.path.contains('\\') || self.path.split('/').any(|s| s.is_empty() || s==".." || s==".")
            || self.path.contains(':') {
            return Err(Error::Invalid("relative source resource path".into()));
        }
        for h in [&self.resource_sha256, &self.dxbc_sha256] {
            if h.len()!=64 || !h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(Error::Invalid("canonical lowercase SHA-256".into()));
            }
        }
        if self.resource_bytes > 128*1024*1024 || self.byte_count<32 || self.byte_count>4*1024*1024
            || self.offset.checked_add(self.byte_count).is_none_or(|e| e>self.resource_bytes) {
            return Err(Error::Invalid("bounded original resource/slice extent".into()));
        }
        Ok(())
    }
}

/// Immutable authenticated source slice. It cannot be constructed from
/// unauthenticated bytes or mutated after hashing.
#[derive(Clone, Debug)]
pub struct SourceShader { identity: SourceIdentity, bytes: Arc<[u8]> }
impl SourceShader {
    pub fn identity(&self) -> &SourceIdentity { &self.identity }
    pub fn bytes(&self) -> &[u8] { &self.bytes }
    pub fn shared_bytes(&self) -> Arc<[u8]> { Arc::clone(&self.bytes) }
    /// Rebuilds an authenticated slice from its identity and the slice bytes alone
    /// (the translator's blob store): the identity is validated and the slice must be the complete
    /// original container it names.
    pub fn from_authenticated_slice(identity: SourceIdentity, bytes: Arc<[u8]>) -> Result<Self> {
        identity.validate()?;
        if bytes.len() as u64 != identity.byte_count || bytes.get(..4) != Some(b"DXBC") || sha256(&bytes) != identity.dxbc_sha256 {
            return Err(Error::Identity("original complete-container slice/hash".into()));
        }
        Ok(Self { identity, bytes })
    }
}

pub fn extract_original(identity: &SourceIdentity, read: impl FnOnce(&str)->Result<Vec<u8>>) -> Result<SourceShader> {
    identity.validate()?;
    let resource=read(&identity.path)?;
    if resource.len() as u64!=identity.resource_bytes || sha256(&resource)!=identity.resource_sha256 {
        if let Some(c)=tolerant_dxbc(&identity.path,&resource,identity.offset as usize,identity.byte_count as usize,&identity.dxbc_sha256) {
            // The identity describes the installed bytes (another D2R build), so every later
            // provenance check compares against what is actually lifted.
            let offset=c.as_ptr() as usize-resource.as_ptr() as usize;
            let actual=SourceIdentity { resource_sha256:sha256(&resource), resource_bytes:resource.len() as u64, offset:offset as u64, byte_count:c.len() as u64, dxbc_sha256:sha256(c), ..identity.clone() };
            return Ok(SourceShader { identity:actual, bytes:Arc::from(c) });
        }
        return Err(Error::Identity("full original resource extent/hash".into()));
    }
    let end=identity.offset.checked_add(identity.byte_count).ok_or_else(||Error::Invalid("slice overflow".into()))?;
    let slice=resource.get(identity.offset as usize..end as usize).ok_or_else(||Error::Invalid("slice extent".into()))?;
    if slice.get(..4)!=Some(b"DXBC") || sha256(slice)!=identity.dxbc_sha256 {
        return Err(Error::Identity("original complete-container slice/hash".into()));
    }
    Ok(SourceShader { identity:identity.clone(), bytes:Arc::from(slice) })
}

