//! Deterministic immutable preparation artifacts, never old compiler identities.
use crate::{ADAPTER_REVISION, Error, Result, SourceIdentity, sha256};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

const MAX_CACHE_BYTES: u64 = 32 * 1024 * 1024;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

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
impl CacheIdentity {
    /// Construct after source authentication and explicit caller/link validation.
    pub(crate) fn current(
        source: SourceIdentity,
        contract: &Value,
        linkage: &Value,
        target: &Value,
        manifest: &Value,
        passes: BTreeMap<String, u32>,
    ) -> Result<Self> {
        Ok(Self {
            schema: "d2r-iridia104-cache-v7".into(),
            source,
            adapter_revision: ADAPTER_REVISION.into(),
            dependency_lock_sha256: sha256(include_bytes!("../Cargo.lock")),
            build_identity_qualification: crate::build_identity::identity().0.into(),
            consumed_build_identity_sha256: crate::build_identity::identity().1.into(),
            iridia_version: "0.5.0".into(),
            decoder_version: "0.5.0+d2r-wrist648-lifter22".into(),
            iridia_naga_version: "30.0.1".into(),
            stock_naga_version: "30.0.1".into(),
            precision_receipt_revision: iridia::precision::RECEIPT_REVISION,
            precision_policy_revision: iridia::precision::POLICY_REVISION,
            program_version: iridia::PROGRAM_VERSION,
            lifter_version: iridia_dxbc::LIFTER_VERSION,
            passes,
            caller_contract_sha256: sha256(&canonical_bytes(contract)?),
            linked_interface_sha256: sha256(&canonical_bytes(linkage)?),
            target_capabilities_sha256: sha256(&canonical_bytes(target)?),
            manifest_sha256: sha256(&canonical_bytes(manifest)?),
            payload_kind: String::new(),
            payload_sha256: String::new(),
            payload_bytes_sha256: String::new(),
        })
    }
    pub fn key(&self) -> Result<String> {
        Ok(sha256(&canonical_bytes(self)?))
    }
    fn validate_current(&self) -> Result<()> {
        self.source.validate()?;
        if self.schema != "d2r-iridia104-cache-v7"
            || self.adapter_revision != ADAPTER_REVISION
            || self.dependency_lock_sha256 != sha256(include_bytes!("../Cargo.lock"))
            || self.build_identity_qualification != crate::build_identity::identity().0
            || self.consumed_build_identity_sha256 != crate::build_identity::identity().1
            || self.iridia_version != "0.5.0"
            || self.decoder_version != "0.5.0+d2r-wrist648-lifter22"
            || self.iridia_naga_version != "30.0.1"
            || self.stock_naga_version != "30.0.1"
            || self.precision_receipt_revision != iridia::precision::RECEIPT_REVISION
            || self.precision_policy_revision != iridia::precision::POLICY_REVISION
            || self.program_version != iridia::PROGRAM_VERSION
            || self.lifter_version != iridia_dxbc::LIFTER_VERSION
        {
            return Err(Error::Identity("stale Iridia converter identity".into()));
        }
        Ok(())
    }
}

/// Owned renderer boundary. No forked Naga type or platform compiler module.
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
        self.validate()
    }
    pub(crate) fn checked(
        identity: CacheIdentity,
        caller_contract: Value,
        linked_interface: Value,
        target_capabilities: Value,
        manifest: Value,
        wgsl: String,
    ) -> Result<Self> {
        Self::checked_payload(identity, caller_contract, linked_interface, target_capabilities, manifest, crate::payload104::ShaderPayload104::Wgsl { text: wgsl.into() })
    }
    pub(crate) fn checked_payload(mut identity: CacheIdentity, caller_contract: Value, linked_interface: Value, target_capabilities: Value, manifest: Value, payload: crate::payload104::ShaderPayload104) -> Result<Self> {
        identity.payload_kind = payload.kind().into();
        identity.payload_sha256 = payload.digest();
        identity.payload_bytes_sha256 = payload.bytes_sha256();
        let s = Self {
            identity,
            caller_contract,
            linked_interface,
            target_capabilities,
            manifest,
            payload,
        };
        s.validate()?;
        Ok(s)
    }
    fn validate(&self) -> Result<()> {
        self.identity.validate_current()?;
        if matches!(self.payload,crate::payload104::ShaderPayload104::AuthoredSpirv{..}) {return Err(Error::Unsupported("authored fixture is not source preparation".into()))}
        if self.identity.payload_sha256 != self.payload.digest()
            || self.identity.payload_kind != self.payload.kind()
            || self.identity.payload_bytes_sha256 != self.payload.bytes_sha256()
            || self.identity.caller_contract_sha256
                != sha256(&canonical_bytes(&self.caller_contract)?)
            || self.identity.linked_interface_sha256
                != sha256(&canonical_bytes(&self.linked_interface)?)
            || self.identity.target_capabilities_sha256
                != sha256(&canonical_bytes(&self.target_capabilities)?)
            || self.identity.manifest_sha256 != sha256(&canonical_bytes(&self.manifest)?)
        {
            return Err(Error::Identity("prepared payload/contract checksum".into()));
        }
        self.payload.validate()?;
        if matches!(self.payload,crate::payload104::ShaderPayload104::ControlledSpirv{..}) && self.payload.source_sha256()!=Some(self.identity.source.dxbc_sha256.as_str()) {return Err(Error::Identity("precision receipt source differs".into()))}
        crate::validation::validate_prepared(self)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    schema: String,
    payload_sha256: String,
    payload: PreparedShader,
}

fn cache_path(root: &Path, id: &CacheIdentity) -> Result<PathBuf> {
    if !matches!(id.payload_kind.as_str(),"wgsl"|"controlled-spirv104") || id.payload_sha256.len()!=64 || !id.payload_sha256.bytes().all(|b|b.is_ascii_hexdigit()){return Err(Error::Identity("cache payload kind/digest".into()))}
    Ok(root.join(format!("{}.iridia50.json", id.key()?)))
}

/// Publish one checksummed payload/metadata artifact atomically, without replacing
/// existing evidence. Filesystems lacking hard-link publication fail explicitly.
pub fn store_cache(root: &Path, prepared: &PreparedShader) -> Result<PathBuf> {
    prepared.validate()?;
    fs::create_dir_all(root)?;
    let destination = cache_path(root, &prepared.identity)?;
    if destination.exists() {
        let old = load_cache(root, &prepared.identity)?;
        if &old != prepared {
            return Err(Error::Identity(
                "same identity has different immutable payload".into(),
            ));
        }
        return Ok(destination);
    }
    let e = Envelope {
        schema: "d2r-iridia104-envelope-v3".into(),
        payload_sha256: sha256(&canonical_bytes(prepared)?),
        payload: prepared.clone(),
    };
    let bytes = canonical_bytes(&e)?;
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return Err(Error::Invalid("cache publication extent".into()));
    }
    let n = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let temp = root.join(format!(".iridia50-{}-{n}.tmp", std::process::id()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        match fs::hard_link(&temp, &destination) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if load_cache(root, &prepared.identity)? != *prepared {
                    return Err(Error::Identity(
                        "concurrent immutable publication differs".into(),
                    ));
                }
                Ok(())
            }
            Err(e) => Err(Error::Io(format!(
                "atomic hard-link cache publication: {e}"
            ))),
        }
    })();
    // Only the uniquely owned temporary file is removed, never a cache artifact.
    let _ = fs::remove_file(&temp);
    result?;
    Ok(destination)
}

pub fn load_cache(root: &Path, expected: &CacheIdentity) -> Result<PreparedShader> {
    expected.validate_current()?;
    let p = cache_path(root, expected)?;
    let file = fs::File::open(&p)?;
    if file.metadata()?.len() > MAX_CACHE_BYTES {
        return Err(Error::Invalid("cache load extent".into()));
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    file.take(MAX_CACHE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CACHE_BYTES {
        return Err(Error::Invalid("cache changed extent".into()));
    }
    let e: Envelope = serde_json::from_slice(&bytes)?;
    if e.schema != "d2r-iridia104-envelope-v3"
        || &e.payload.identity != expected
        || e.payload_sha256 != sha256(&canonical_bytes(&e.payload)?)
    {
        return Err(Error::Identity(
            "cache envelope/source/converter identity".into(),
        ));
    }
    e.payload.validate()?;
    Ok(e.payload)
}

/// Weak interning: immutable WGSL/metadata allocations are shared by every live
/// instance and disappear when the final owner releases them. No per-draw hash.
pub struct SharedPreparedCache {
    entries: BTreeMap<String, Weak<PreparedShader>>,
    max_entries: usize,
    max_live_bytes: usize,
}
impl SharedPreparedCache {
    pub fn new(max_entries: usize, max_live_bytes: usize) -> Result<Self> {
        if max_entries == 0 || max_entries > 65536 || max_live_bytes == 0 {
            return Err(Error::Invalid("prepared cache budget".into()));
        }
        Ok(Self {
            entries: BTreeMap::new(),
            max_entries,
            max_live_bytes,
        })
    }
    pub fn intern(&mut self, p: PreparedShader) -> Result<Arc<PreparedShader>> {
        p.validate()?;
        let key = p.identity.key()?;
        if let Some(existing) = self.entries.get(&key).and_then(Weak::upgrade) {
            if *existing != p {
                return Err(Error::Identity(
                    "same preparation key has different live payload".into(),
                ));
            }
            return Ok(existing);
        }
        self.entries.retain(|_, v| v.strong_count() != 0);
        let live = self
            .entries
            .values()
            .filter_map(Weak::upgrade)
            .try_fold(0usize, |n, p| {
                n.checked_add(serde_json::to_vec(&*p).ok()?.len())
            })
            .ok_or_else(|| Error::Invalid("prepared cache byte accounting overflow".into()))?;
        let size = serde_json::to_vec(&p)?.len();
        if self.entries.len() >= self.max_entries
            || live
                .checked_add(size)
                .is_none_or(|b| b > self.max_live_bytes)
        {
            return Err(Error::Invalid("prepared cache budget exhausted".into()));
        }
        let shared = Arc::new(p);
        self.entries.insert(key, Arc::downgrade(&shared));
        Ok(shared)
    }
}

