//! Cross-process calls into the GPL translator program `d2r-shader-translator`.
//!
//! A call is a [`Request`] (function name and arguments; byte inputs by SHA-256 of a blob). Its key is
//! the SHA-256 of the canonical request JSON. Layout under the cache root ([`cache_root`]):
//!
//! - `blobs/<sha256>`: input bytes (original DXBC slices, terrain sources, host grids, artifacts);
//! - `t-<translator cache id, 16 hex>/<key[0..2]>/<key>.req.json` and `<key>.res.json` (transid1177:
//!   the id comes from the translation inputs, `d2r-shader-translator cache-id`, not the program bytes);
//! - `requests.log`: one `t-dir/kk/key` line per request this PC made (the warm-up list).
//!
//! The result file holds the function's own result (`Ok` value or its error), so a refusal replays
//! exactly as in process. A missing translator, a failed run or a result that does not re-serialize
//! to the same JSON is a transport error (`PRETRANS1177 ERROR ...`) and is never cached. There is
//! no in-process, vkd3d or direct-D3D12 substitute.
use crate::caller_projection::{CallerEvidence, NullBufferFixture, NullImageFixture};
use crate::resource_contract::CallerContract;
use crate::stage_link::DrawContract;
use crate::{Error, SourceIdentity, SourceShader, TargetContract, sha256};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Wire schema; part of every key.
pub const SCHEMA: &str = "pretrans1177-rpc-v1";
pub const TRANSLATOR_ENV: &str = "D2R_TRANSLATOR1177";
pub const CACHE_ENV: &str = "D2R_TRANSLATED1177";
pub const LOG_ENV: &str = "D2R_PRETRANS1177_LOG";
pub fn translator_file_name() -> String {
    format!("d2r-shader-translator{}", std::env::consts::EXE_SUFFIX)
}

/// An authenticated original slice on the wire: its identity and the blob of its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireSource {
    pub identity: SourceIdentity,
    pub bytes: String,
}

/// How the translator rebuilds a capability: the seal arguments (bytes by blob).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cap", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapRecipe {
    Caller { profile: String, fixtures: Vec<NullImageFixture>, unit_t4: Option<String> },
    Waves { images: Vec<NullImageFixture>, buffers: Vec<NullBufferFixture> },
    Foliage { vs: WireSource, ps: WireSource, fixtures: Vec<NullImageFixture>, t4: String, units: Option<(String, String)> },
    MapAlpha { vs: WireSource, ps: WireSource, null_images: Vec<u32> },
    MapStandard { vs: WireSource, ps: WireSource, null_images: Vec<u32> },
    MapVariant { vs: WireSource, ps: WireSource, null_images: Vec<u32> },
    /// The `projection()` caller capability of the capability `of`.
    Projection { of: Box<CapRecipe> },
}

/// The public values a seal produced (each capability fills the ones its type exposes).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SealInfo {
    pub evidence: Option<CallerEvidence>,
    pub consumed_t4: Option<[u8; 4]>,
    pub unit_lights: Option<crate::caller_projection::foliage_policy883::UnitLights>,
    pub receipt: Option<Value>,
    pub receiver: Option<bool>,
    pub null_images: Option<Vec<u32>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "fn", content = "args", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Identity,
    InspectSource { source: WireSource },
    PreparePreciseSpirv { source: WireSource },
    DiagnosticWgsl { source: WireSource },
    PrepareCompute { controlled: bool, profile: String, source: WireSource, caller: CallerContract, target: TargetContract },
    PrepareGraphics {
        controlled: bool,
        profile: String,
        vs: WireSource,
        ps: WireSource,
        vertex: CallerContract,
        fragment: CallerContract,
        draw: DrawContract,
        target: TargetContract,
        projection: Option<CapRecipe>,
    },
    PrepareStandardMap { vs: WireSource, ps: WireSource, vertex: CallerContract, physical_fragment: CallerContract, draw: DrawContract, target: TargetContract, cap: CapRecipe },
    PrepareFoliage { vs: WireSource, ps: WireSource, vertex: CallerContract, physical_fragment: CallerContract, draw: DrawContract, target: TargetContract, cap: CapRecipe },
    ProjectedCallerContract { source: WireSource, caller: CallerContract, cap: CapRecipe },
    /// `artifact` = blob of the artifact's JSON.
    ValidateShader { artifact: String },
    ValidateGraphics { artifact: String },
    Seal { cap: CapRecipe },
    /// Terrain53: `files` = source path -> blob.
    TerrainLoad { files: BTreeMap<String, String> },
    TerrainPrefix { files: BTreeMap<String, String> },
    /// `prefix` = blob of the prefix wire JSON ([`crate::terrain_iridia::PrefixWire`]).
    TerrainValidatePrefix { files: BTreeMap<String, String>, prefix: String },
    /// `witness` = path of the scene40 witness manifest (read by the translator; never cached).
    TerrainWitness { files: BTreeMap<String, String>, witness: String },
    TerrainPixel { files: BTreeMap<String, String>, witness: Option<String>, shadow: bool },
}
impl Request {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::InspectSource { .. } => "inspect_source",
            Self::PreparePreciseSpirv { .. } => "prepare_precise_spirv",
            Self::DiagnosticWgsl { .. } => "diagnostic_wgsl",
            Self::PrepareCompute { .. } => "prepare_compute",
            Self::PrepareGraphics { .. } => "prepare_graphics",
            Self::PrepareStandardMap { .. } => "prepare_standard_map",
            Self::PrepareFoliage { .. } => "prepare_foliage",
            Self::ProjectedCallerContract { .. } => "projected_caller_contract",
            Self::ValidateShader { .. } => "validate_shader",
            Self::ValidateGraphics { .. } => "validate_graphics",
            Self::Seal { .. } => "seal",
            Self::TerrainLoad { .. } => "terrain_load",
            Self::TerrainPrefix { .. } => "terrain_prefix",
            Self::TerrainValidatePrefix { .. } => "terrain_validate_prefix",
            Self::TerrainWitness { .. } => "terrain_witness",
            Self::TerrainPixel { .. } => "terrain_pixel",
        }
    }
    /// Requests that read files outside the blob store are never cached.
    pub fn cacheable(&self) -> bool {
        !matches!(self, Self::TerrainWitness { .. } | Self::TerrainPixel { witness: Some(_), .. })
    }
}

/// The request file: schema + request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    pub schema: String,
    pub request: Request,
}

/// A function's error on the wire (the crate `Error` variants, a plain message for functions that
/// return `Result<_, String>`, the terrain `ShaderError` variants).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "message", deny_unknown_fields)]
pub enum WireError {
    Invalid(String),
    Identity(String),
    Unsupported(String),
    Validation(String),
    Io(String),
    Message(String),
    ShaderInvalid(String),
    ShaderUnsupported(String),
    ShaderValidation(String),
    ShaderIo(String),
}
impl From<Error> for WireError {
    fn from(e: Error) -> Self {
        match e {
            Error::Invalid(s) => Self::Invalid(s),
            Error::Identity(s) => Self::Identity(s),
            Error::Unsupported(s) => Self::Unsupported(s),
            Error::Validation(s) => Self::Validation(s),
            Error::Io(s) => Self::Io(s),
        }
    }
}
impl From<d2r_shader::ShaderError> for WireError {
    fn from(e: d2r_shader::ShaderError) -> Self {
        use d2r_shader::ShaderError as S;
        match e {
            S::Invalid(s) => Self::ShaderInvalid(s),
            S::Unsupported(s) => Self::ShaderUnsupported(s),
            S::Validation(s) => Self::ShaderValidation(s),
            S::Io(e) => Self::ShaderIo(e.to_string()),
        }
    }
}
impl WireError {
    pub fn into_error(self) -> Error {
        match self {
            Self::Invalid(s) => Error::Invalid(s),
            Self::Identity(s) => Error::Identity(s),
            Self::Unsupported(s) => Error::Unsupported(s),
            Self::Validation(s) => Error::Validation(s),
            Self::Io(s) => Error::Io(s),
            other => Error::Io(format!("PRETRANS1177 ERROR: unexpected error form {other:?}")),
        }
    }
    pub fn into_message(self) -> String {
        match self {
            Self::Message(s) => s,
            other => format!("PRETRANS1177 ERROR: unexpected error form {other:?}"),
        }
    }
    pub fn into_shader(self) -> d2r_shader::ShaderError {
        use d2r_shader::ShaderError as S;
        match self {
            Self::ShaderInvalid(s) => S::Invalid(s),
            Self::ShaderUnsupported(s) => S::Unsupported(s),
            Self::ShaderValidation(s) => S::Validation(s),
            Self::ShaderIo(s) => S::Io(std::io::Error::other(s)),
            other => S::Io(std::io::Error::other(format!("PRETRANS1177 ERROR: unexpected error form {other:?}"))),
        }
    }
}

/// The result file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Response {
    Ok(Value),
    Err(WireError),
}

/// The translator's identity answer (`Request::Identity`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslatorIdentity {
    pub adapter_revision: String,
    pub compute_revision: String,
    pub consumed_build_identity: Option<String>,
    pub consumed_build_identity_sha256: Option<String>,
    pub require_consumed_build_identity: Result<String, WireError>,
    pub terrain_consumed_identity58: Result<(String, String), String>,
    pub qualification_plan: String,
}

// ------------------------------------------------------------------ canonical JSON and blobs --

fn sorted(v: Value) -> Value {
    match v {
        Value::Object(map) => Value::Object(map.into_iter().map(|(k, v)| (k, sorted(v))).collect::<BTreeMap<_, _>>().into_iter().collect()),
        Value::Array(a) => Value::Array(a.into_iter().map(sorted).collect()),
        v => v,
    }
}
/// Canonical (sorted-key, compact) JSON bytes.
pub fn canonical_json(value: &impl Serialize) -> std::result::Result<Vec<u8>, String> {
    serde_json::to_vec(&sorted(serde_json::to_value(value).map_err(|e| e.to_string())?)).map_err(|e| e.to_string())
}
pub fn request_key(envelope: &Envelope) -> std::result::Result<(Vec<u8>, String), String> {
    let bytes = canonical_json(envelope)?;
    let key = sha256(&bytes);
    Ok((bytes, key))
}

/// `%LOCALAPPDATA%\D2RVS\cache\translated1177` (`D2R_TRANSLATED1177` overrides; temp folder without
/// LOCALAPPDATA). Game-derived: written on this PC only, never shipped.
pub fn cache_root() -> PathBuf {
    if let Some(p) = std::env::var_os(CACHE_ENV).filter(|v| !v.is_empty()) {
        return PathBuf::from(p);
    }
    std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir).join("D2RVS").join("cache").join("translated1177")
}

/// Atomic write: a unique temporary file in the same folder, then rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = path.parent().ok_or_else(|| std::io::Error::other("no parent folder"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("f"),
        std::process::id(),
        N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        // Another process wrote the same content-addressed file first.
        Err(_) if path.exists() => {
            let _ = std::fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

pub fn blob_path(root: &Path, sha: &str) -> PathBuf {
    root.join("blobs").join(sha)
}
/// Stores `bytes` in the blob store and returns their SHA-256.
pub fn blob(bytes: &[u8]) -> std::result::Result<String, String> {
    static WRITTEN: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    let sha = sha256(bytes);
    let written = WRITTEN.get_or_init(Default::default);
    if written.lock().map(|w| w.contains(&sha)).unwrap_or(false) {
        return Ok(sha);
    }
    let path = blob_path(&cache_root(), &sha);
    if !path.exists() {
        write_atomic(&path, bytes).map_err(|e| format!("PRETRANS1177 ERROR: blob {}: {e}", path.display()))?;
    }
    if let Ok(mut w) = written.lock() {
        w.insert(sha.clone());
    }
    Ok(sha)
}
pub fn wire_source(s: &SourceShader) -> std::result::Result<WireSource, String> {
    Ok(WireSource { identity: s.identity().clone(), bytes: blob(s.bytes())? })
}

// ------------------------------------------------------------------------ translator lookup --

struct Translator {
    exe: PathBuf,
    /// The cache id the translator names itself (`cache-id`), 64 hex digits.
    id: String,
}
/// The one line `d2r-shader-translator cache-id` prints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheIdLine {
    pub cache_id: String,
    pub folder: String,
}
/// Parses and checks the `cache-id` output (64 lowercase hex digits, folder `t-<first 16>`).
pub fn parse_cache_id(stdout: &str) -> std::result::Result<String, String> {
    let line: CacheIdLine = serde_json::from_str(stdout.trim()).map_err(|e| format!("cache-id output {:?}: {e}", stdout.trim()))?;
    let hex = line.cache_id.len() == 64 && line.cache_id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    if !hex || line.folder != format!("t-{}", &line.cache_id[..16]) {
        return Err(format!("cache-id output {:?} is not a 64-digit id with its folder", stdout.trim()));
    }
    Ok(line.cache_id)
}
/// Runs `<exe> cache-id` (the translator computes its id from its build inputs and this
/// environment; see the translator's `transid1177.rs`). No fallback: a failure is an error.
pub fn ask_cache_id(exe: &Path) -> std::result::Result<String, String> {
    let call = "cache-id (the translator's cache folder)";
    let out = std::process::Command::new(exe)
        .arg("cache-id")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| failure_block(Some(exe), call, Failure::Start(&e), ""))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(failure_block(Some(exe), call, Failure::Exit(out.status), &stderr));
    }
    parse_cache_id(&String::from_utf8_lossy(&out.stdout)).map_err(|m| failure_block(Some(exe), call, Failure::Result(&m), &stderr))
}
/// `D2R_TRANSLATOR1177`, else `d2r-shader-translator(.exe)` next to this program (for test binaries
/// in `target/<profile>/deps`, also the profile folder above it).
pub fn translator_path() -> std::result::Result<PathBuf, String> {
    if let Some(p) = std::env::var_os(TRANSLATOR_ENV).filter(|v| !v.is_empty()) {
        let p = PathBuf::from(p);
        return if p.is_file() { Ok(p) } else { Err(failure_block(None, "find the translator", Failure::NotFound(&[p]), "")) };
    }
    let me = std::env::current_exe().map_err(|e| format!("PRETRANS1177 ERROR: own program path: {e}"))?;
    let dir = me.parent().ok_or("PRETRANS1177 ERROR: own program folder")?;
    let mut tried = vec![dir.join(translator_file_name())];
    if dir.file_name().is_some_and(|n| n == "deps") {
        if let Some(up) = dir.parent() {
            tried.push(up.join(translator_file_name()));
        }
    }
    tried.iter().find(|p| p.is_file()).cloned().ok_or_else(|| failure_block(None, "find the translator", Failure::NotFound(&tried), ""))
}
fn translator() -> std::result::Result<&'static Translator, String> {
    static T: OnceLock<std::result::Result<Translator, String>> = OnceLock::new();
    T.get_or_init(|| {
        let exe = translator_path()?;
        let asked = std::time::Instant::now();
        let id = ask_cache_id(&exe)?;
        // One start-log line, the folder this process reads and writes.
        eprintln!(
            "PRETRANS1177 cache id lookup: {} cache-id {:.0} ms, folder {}",
            exe.display(),
            asked.elapsed().as_secs_f64() * 1e3,
            translator_dir(&cache_root(), &id).display()
        );
        Ok(Translator { id, exe })
    })
    .as_ref()
    .map_err(|e| {
        report(e);
        e.clone()
    })
}
/// The cache folder of one translator cache id (from the translation inputs, not the
/// program bytes).
pub fn translator_dir(root: &Path, cache_id: &str) -> PathBuf {
    root.join(format!("t-{}", &cache_id[..16]))
}
pub fn request_paths(dir: &Path, key: &str) -> (PathBuf, PathBuf) {
    let d = dir.join(&key[..2]);
    (d.join(format!("{key}.req.json")), d.join(format!("{key}.res.json")))
}

/// Every transport error is printed once per distinct message.
fn report(msg: &str) {
    static SEEN: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
    if SEEN.get_or_init(Default::default).lock().map(|mut s| s.insert(msg.to_owned())).unwrap_or(true) {
        eprintln!("{msg}");
    }
}

/// Counters for the start-up display (the second start-up stage shows them during the logo).
pub static HITS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static MISSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Runs one request: cache hit, else one translator run. Outer `Err` = transport error.
pub fn call_raw(request: Request) -> std::result::Result<Response, String> {
    let t = translator()?;
    let root = cache_root();
    let envelope = Envelope { schema: SCHEMA.into(), request };
    let (bytes, key) = request_key(&envelope)?;
    let dir = translator_dir(&root, &t.id);
    let (req, res) = request_paths(&dir, &key);
    let cacheable = envelope.request.cacheable();
    if cacheable {
        if let Ok(b) = std::fs::read(&res) {
            if let Ok(r) = serde_json::from_slice::<Response>(&b) {
                HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(r);
            }
            report(&format!("PRETRANS1177 WARN: unreadable cached result {} is translated again", res.display()));
        }
    }
    // One translation per request at a time across threads and processes (the warm-up included):
    // a waiter reads the result its holder wrote.
    let _lock = if cacheable {
        match Lock::take(&res) {
            Taken::Owned(l) => Some(l),
            Taken::Written(b) => {
                if let Ok(r) = serde_json::from_slice::<Response>(&b) {
                    HITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(r);
                }
                None
            }
        }
    } else {
        None
    };
    MISSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let fail = |m: String| {
        report(&m);
        m
    };
    write_atomic(&req, &bytes).map_err(|e| fail(format!("PRETRANS1177 ERROR: request {}: {e}", req.display())))?;
    if cacheable {
        let line = format!("{}/{}/{key}\n", dir.file_name().and_then(|n| n.to_str()).unwrap_or_default(), &key[..2]);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(root.join("requests.log")) {
            let _ = f.write_all(line.as_bytes());
        }
    }
    let started = std::time::Instant::now();
    let what = format!("{} (request {})", envelope.request.name(), req.display());
    let out = std::process::Command::new(&t.exe)
        .arg("run")
        .arg(&root)
        .arg(&req)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| fail(failure_block(Some(&t.exe), &what, Failure::Start(&e), "")))?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(fail(failure_block(Some(&t.exe), &what, Failure::Exit(out.status), &stderr)));
    }
    // The translator's own warnings (for example MapStandard1129) still reach this console.
    eprint!("{stderr}");
    let r = std::fs::read(&res)
        .map_err(|e| format!("result file {}: {e}", res.display()))
        .and_then(|b| serde_json::from_slice::<Response>(&b).map_err(|e| format!("result file {}: {e}", res.display())))
        .map_err(|m| fail(failure_block(Some(&t.exe), &what, Failure::Result(&m), &stderr)))?;
    if !cacheable {
        let _ = std::fs::remove_file(&res);
    }
    if std::env::var_os(LOG_ENV).is_some_and(|v| v == "1") {
        eprintln!("PRETRANS1177 {} {} {:.0} ms", envelope.request.name(), &key[..12], started.elapsed().as_secs_f64() * 1e3);
    }
    Ok(r)
}

/// Decodes an `Ok` value into `T` and refuses a value that does not re-serialize to the same JSON
/// (a schema difference between this crate and the translator is an error, never silent).
pub fn decode<T: DeserializeOwned + Serialize>(name: &str, v: Value) -> std::result::Result<T, String> {
    let t: T = serde_json::from_value(v.clone()).map_err(|e| format!("PRETRANS1177 ERROR: {name} result form: {e}"))?;
    let back = serde_json::to_value(&t).map_err(|e| format!("PRETRANS1177 ERROR: {name} result form: {e}"))?;
    if back != v {
        return Err(format!("PRETRANS1177 ERROR: {name} result does not re-serialize to the translator's JSON (schema drift)"));
    }
    Ok(t)
}

/// One call with the function's error mapped by `err` and a transport error by `transport`.
pub fn call<T: DeserializeOwned + Serialize, E>(request: Request, err: impl FnOnce(WireError) -> E, transport: impl FnOnce(String) -> E) -> std::result::Result<T, E> {
    let name = request.name();
    match call_raw(request) {
        Ok(Response::Ok(v)) => decode(name, v).map_err(|m| {
            report(&m);
            transport(m)
        }),
        Ok(Response::Err(e)) => Err(err(e)),
        Err(m) => Err(transport(m)),
    }
}
/// A call of a function that returns the crate `Error`.
pub fn call_e<T: DeserializeOwned + Serialize>(request: Request) -> crate::Result<T> {
    call(request, WireError::into_error, Error::Io)
}
pub fn call_s<T: DeserializeOwned + Serialize>(request: Request) -> std::result::Result<T, String> {
    call(request, WireError::into_message, |m| m)
}
pub fn call_shader<T: DeserializeOwned + Serialize>(request: Request) -> std::result::Result<T, d2r_shader::ShaderError> {
    call(request, WireError::into_shader, |m| d2r_shader::ShaderError::Io(std::io::Error::other(m)))
}
pub(crate) fn src(s: &SourceShader) -> crate::Result<WireSource> {
    wire_source(s).map_err(Error::Io)
}

/// The translator identity (one call per process; cached like every request).
pub fn identity() -> crate::Result<Arc<TranslatorIdentity>> {
    static ID: OnceLock<std::result::Result<Arc<TranslatorIdentity>, Error>> = OnceLock::new();
    ID.get_or_init(|| {
        let id: TranslatorIdentity = call_e(Request::Identity)?;
        if id.adapter_revision != crate::ADAPTER_REVISION || id.compute_revision != crate::COMPUTE_REVISION {
            let m = format!("PRETRANS1177 ERROR: translator adapter {} / compute {} differ from this viewer's {} / {}", id.adapter_revision, id.compute_revision, crate::ADAPTER_REVISION, crate::COMPUTE_REVISION);
            report(&m);
            return Err(Error::Io(m));
        }
        Ok(Arc::new(id))
    })
    .clone()
}

pub(crate) fn validate_shader(p: &crate::PreparedShader) -> crate::Result<()> {
    let artifact = blob(&serde_json::to_vec(p)?).map_err(Error::Io)?;
    call_e(Request::ValidateShader { artifact })
}
pub(crate) fn validate_graphics(p: &crate::PreparedGraphics) -> crate::Result<()> {
    let artifact = blob(&serde_json::to_vec(p)?).map_err(Error::Io)?;
    call_e(Request::ValidateGraphics { artifact })
}

// ------------------------------------------------------------------------------ request locks --

/// A lock older than this is taken over: only a backstop for lock files that outlive their holder
/// (older translator builds, systems without delete-on-close).
pub const STALE_LOCK_SECONDS: u64 = 300;
pub fn lock_path(res: &Path) -> PathBuf {
    res.with_extension("lock")
}
/// Creates the lock file of one request (create-new). On Windows the handle is opened with
/// `FILE_FLAG_DELETE_ON_CLOSE`: the file disappears with its holder, also when the holder process
/// crashes or is stopped, so a stopped warm-up leaves no lock behind.
fn create_lock(path: &Path) -> std::io::Result<std::fs::File> {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        const DELETE: u32 = 0x0001_0000;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_DELETE: u32 = 0x4;
        const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
        o.access_mode(GENERIC_WRITE | DELETE).share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE).custom_flags(FILE_FLAG_DELETE_ON_CLOSE);
    }
    let mut f = o.open(path)?;
    let _ = write!(f, "{}", std::process::id());
    Ok(f)
}
/// The lock of one request (`<key>.res.lock`), removed on drop (Windows: when its handle closes).
pub struct Lock(PathBuf, Option<std::fs::File>);
pub enum Taken {
    Owned(Lock),
    /// Another holder wrote the result meanwhile (its bytes).
    Written(Vec<u8>),
}
impl Lock {
    /// Waits until this caller holds the request's lock or its result exists.
    pub fn take(res: &Path) -> Taken {
        let path = lock_path(res);
        loop {
            if let Ok(b) = std::fs::read(res) {
                return Taken::Written(b);
            }
            match create_lock(&path) {
                Ok(f) => {
                    let lock = Lock(path.clone(), Some(f));
                    // The holder may have finished between the result check and the lock.
                    if let Ok(b) = std::fs::read(res) {
                        return Taken::Written(b);
                    }
                    return Taken::Owned(lock);
                }
                // Access denied (5): the file is delete-pending, its holder is releasing it.
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists || e.raw_os_error() == Some(5) => {
                    let stale = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .ok()
                        .and_then(|t| t.elapsed().ok())
                        .is_some_and(|age| age.as_secs() > STALE_LOCK_SECONDS);
                    if stale {
                        report(&format!("PRETRANS1177 WARN: stale request lock {} taken over", path.display()));
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                // No lock possible (read-only folder): translate without one.
                Err(_) => return Taken::Owned(Lock(PathBuf::new(), None)),
            }
        }
    }
    /// Non-blocking: `None` when another holder has the lock or the result exists.
    pub fn try_take(res: &Path) -> Option<Lock> {
        if res.exists() {
            return None;
        }
        let path = lock_path(res);
        let lock = Lock(path.clone(), Some(create_lock(&path).ok()?));
        if res.exists() {
            return None;
        }
        Some(lock)
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let delete_on_close = cfg!(windows) && self.1.is_some();
        drop(self.1.take());
        if !delete_on_close && !self.0.as_os_str().is_empty() {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

// ---------------------------------------------------------------------------- failure text --

/// What went wrong with the translator program (see [`failure_block`]).
pub enum Failure<'a> {
    NotFound(&'a [PathBuf]),
    Read(&'a std::io::Error),
    Start(&'a std::io::Error),
    Exit(std::process::ExitStatus),
    Result(&'a str),
}
/// Lines of translator stderr shown in a failure block.
pub const STDERR_LINES: usize = 12;

/// The exit of a failed run in words, and whether it was a crash or panic.
pub fn exit_text(status: std::process::ExitStatus) -> (String, bool) {
    let Some(code) = status.code() else { return (format!("ended by a signal ({status})"), true) };
    let u = code as u32;
    if u >= 0xC000_0000 {
        let name = match u {
            0xC000_0005 => " (access violation)",
            0xC000_00FD => " (stack overflow)",
            0xC000_0409 => " (fail-fast / abort)",
            0xC000_0374 => " (heap corruption)",
            0xC000_0017 => " (out of memory)",
            0xC000_013A => " (stopped with Ctrl+C)",
            0xC000_0142 => " (DLL initialisation failed)",
            _ => "",
        };
        return (format!("crashed with status 0x{u:08X}{name}"), true);
    }
    match code {
        2 => (format!("exit code {code} (the translator refused the request; its message is below)"), false),
        101 => (format!("exit code {code} (a panic in the translator)"), true),
        _ => (format!("exit code {code}"), false),
    }
}
fn os_hint(e: &std::io::Error) -> Option<&'static str> {
    match e.raw_os_error() {
        // Access denied, virus infected / deleted, blocked by group policy, application control.
        Some(5 | 225 | 226 | 1260 | 4551 | 4556) => Some(
            "Windows refused the file. Antivirus software or an application-control policy (Defender, Smart App Control) may block it: allow d2r-shader-translator.exe or restore it from quarantine.",
        ),
        Some(193 | 216 | 1392) => Some("The file is not a valid Windows program (damaged or incomplete): extract the zip again."),
        Some(2 | 3) => Some("The file is gone (an antivirus quarantine removes files): restore it or extract the zip again."),
        _ => None,
    }
}
/// One readable console block for the user: the program path(s), the call, what happened, the
/// first stderr lines and a hint. The first line starts with `PRETRANS1177 ERROR`, the others
/// with two spaces.
pub fn failure_block(exe: Option<&Path>, call: &str, failure: Failure<'_>, stderr: &str) -> String {
    let mut s = String::from("PRETRANS1177 ERROR: shader translation failed\n");
    match (&failure, exe) {
        (Failure::NotFound(tried), _) => {
            s += "  translator: not found; tried\n";
            for p in tried.iter() {
                s += &format!("    {}\n", p.display());
            }
        }
        (_, Some(e)) => s += &format!("  translator: {}\n", e.display()),
        (_, None) => {}
    }
    s += &format!("  call:       {call}\n");
    let (problem, hint) = match &failure {
        Failure::NotFound(_) => (
            format!("{} is missing", translator_file_name()),
            Some(format!("Put {} from the same zip next to the viewer, or set {TRANSLATOR_ENV}=<path>. There is no built-in fallback.", translator_file_name())),
        ),
        Failure::Read(e) => (format!("the program file cannot be read: {e}"), os_hint(e).map(str::to_owned)),
        Failure::Start(e) => (format!("the program did not start: {e}"), os_hint(e).map(str::to_owned)),
        Failure::Exit(st) => {
            let (t, crash) = exit_text(*st);
            (t, crash.then(|| "The translator crashed: please send this whole block.".to_owned()))
        }
        Failure::Result(m) => (format!("no usable result: {m}"), None),
    };
    s += &format!("  problem:    {problem}\n");
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    if !lines.is_empty() {
        s += &format!("  stderr ({} of {} lines):\n", lines.len().min(STDERR_LINES), lines.len());
        for l in lines.iter().take(STDERR_LINES) {
            s += &format!("    {l}\n");
        }
    } else if matches!(failure, Failure::Exit(_)) {
        s += "  stderr:     (empty)\n";
    }
    if let Some(h) = hint {
        s += &format!("  hint:       {h}\n");
    }
    s.pop();
    s
}

// --------------------------------------------------------------------------------- warm-up --

pub const WARM_ENV: &str = "D2R_PRETRANS1177_WARM";
/// `warm <root> --until-stdin-closes`: the warm-up ends when its standard input reaches its end
/// (the viewer holds the other end).
pub const WARM_PARENT_FLAG: &str = "--until-stdin-closes";
/// Second start-up stage: starts `d2r-shader-translator warm <cache root>` in the background (below-normal
/// priority) so the requests of earlier sessions are translated while the logo plays. The viewer's
/// own requests wait on a request another process is translating instead of repeating it.
/// `D2R_PRETRANS1177_WARM=0` turns it off. A missing translator is reported here once.
pub fn start_warm() {
    if std::env::var_os(WARM_ENV).is_some_and(|v| v == "0") {
        eprintln!("PRETRANS1177 warm-up off ({WARM_ENV}=0)");
        return;
    }
    let Ok(t) = translator() else { return };
    let root = cache_root();
    let logged = std::fs::read_to_string(root.join("requests.log")).map(|l| l.lines().count()).unwrap_or(0);
    let mut cmd = std::process::Command::new(&t.exe);
    // The pipe's write end stays open in this process until it ends; its closing (normal exit,
    // crash or a stop from the task manager alike) ends the warm-up.
    cmd.arg("warm").arg(&root).arg(WARM_PARENT_FLAG).stdin(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(BELOW_NORMAL_PRIORITY_CLASS);
    }
    match cmd.spawn() {
        Ok(mut child) => {
            eprintln!("PRETRANS1177 warm-up started: {} (pid {}), {logged} logged requests, translator t-{}", t.exe.display(), child.id(), &t.id[..16]);
            // Kept until this process ends (never written to).
            static PIPE: Mutex<Vec<std::process::ChildStdin>> = Mutex::new(Vec::new());
            if let (Some(pipe), Ok(mut held)) = (child.stdin.take(), PIPE.lock()) {
                held.push(pipe);
            }
        }
        Err(e) => report(&failure_block(Some(&t.exe), "start the warm-up", Failure::Start(&e), "")),
    }
}
/// One line for the start log: requests answered from the cache and translated in this process.
pub fn summary() -> String {
    format!(
        "PRETRANS1177 shader translations: {} cached, {} translated",
        HITS.load(std::sync::atomic::Ordering::Relaxed),
        MISSES.load(std::sync::atomic::Ordering::Relaxed)
    )
}

