//! `d2r-shader-translator`, the GPL program that translates the original D2R shaders.
//!
//! The viewer (MIT, `d2r-shader-client`) writes a request file and runs
//! `d2r-shader-translator run <cache root> <request file>`. This program reads the request, rebuilds
//! the GPL arguments from it (original slices from the blob store, contracts by their serde form,
//! capabilities by re-sealing their recipe), runs the unchanged function of `d2r-shader-iridia` /
//! `d2r-iridia-terrain53` and writes its result next to the request (temporary file + rename).
//!
//! Every conversion between the client's types and the GPL types is checked: the converted value
//! must serialize to the same JSON as its source, so a schema difference is an error, never silent.
//!
//! Commands:
//! - `run <cache root> <request.req.json>`: one request (exit 0 = result written);
//! - `identity`: prints this program's identity answer and its SHA-256 folder name;
//! - `warm <cache root>`: translates every logged request of this PC that has no result for this
//!   program build yet (the viewer starts it in the background during the logo).
//!
//! Output is game-derived data written on the user's PC only; it is never shipped.

use d2r_iridia_terrain53::terrain_iridia as t53;
use d2r_shader_client as c;
use d2r_shader_iridia as g;
use c::rpc::{self, CapRecipe, Envelope, Request, Response, SealInfo, WireError, WireSource};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod transid1177;
mod built1177 {
    include!(concat!(env!("OUT_DIR"), "/translation_build1177.rs"));
}

type R<T> = Result<T, WireError>;

fn transport(m: impl std::fmt::Display) -> String {
    format!("PRETRANS1177 ERROR (translator): {m}")
}

/// GPL error -> wire.
fn gerr(e: g::Error) -> WireError {
    match e {
        g::Error::Invalid(s) => WireError::Invalid(s),
        g::Error::Identity(s) => WireError::Identity(s),
        g::Error::Unsupported(s) => WireError::Unsupported(s),
        g::Error::Validation(s) => WireError::Validation(s),
        g::Error::Io(s) => WireError::Io(s),
    }
}

/// Converts between the client's and the GPL definitions of one type through its serde form and
/// requires the result to serialize to the same JSON.
fn conv<A: Serialize, B: Serialize + DeserializeOwned>(what: &str, a: &A) -> Result<B, String> {
    let v = serde_json::to_value(a).map_err(|e| transport(format!("{what}: {e}")))?;
    let b: B = serde_json::from_value(v.clone()).map_err(|e| transport(format!("{what}: {e}")))?;
    if serde_json::to_value(&b).map_err(|e| transport(format!("{what}: {e}")))? != v {
        return Err(transport(format!("{what}: client and translator serde forms differ")));
    }
    Ok(b)
}
fn value(what: &str, a: &impl Serialize) -> Result<Value, String> {
    serde_json::to_value(a).map_err(|e| transport(format!("{what}: {e}")))
}

struct Ctx {
    root: PathBuf,
}
impl Ctx {
    fn blob(&self, sha: &str) -> Result<Vec<u8>, String> {
        if sha.len() != 64 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(transport(format!("blob name {sha:?}")));
        }
        let p = rpc::blob_path(&self.root, sha);
        let b = std::fs::read(&p).map_err(|e| transport(format!("blob {}: {e}", p.display())))?;
        if c::sha256(&b) != sha {
            return Err(transport(format!("blob {} content differs from its name", p.display())));
        }
        Ok(b)
    }
    fn source(&self, w: &WireSource) -> Result<R<g::SourceShader>, String> {
        let identity: g::SourceIdentity = conv("source identity", &w.identity)?;
        let bytes: Arc<[u8]> = self.blob(&w.bytes)?.into();
        Ok(g::SourceShader::from_authenticated_slice(identity, bytes).map_err(gerr))
    }
    fn files(&self, files: &BTreeMap<String, String>) -> Result<BTreeMap<String, Vec<u8>>, String> {
        files.iter().map(|(k, v)| Ok((k.clone(), self.blob(v)?))).collect()
    }
}

/// A rebuilt capability (owners keep their projection alive).
enum Cap {
    Caller(g::caller_projection::CallerCapability),
    Foliage(g::caller_projection::foliage_policy883::Foliage23Capability),
    MapAlpha(g::caller_projection::map_alpha_threshold1129::MapAlphaThresholdCapability1129),
    MapStandard(g::caller_projection::standard_map1129::MapStandardCapability1129),
    MapVariant(g::caller_projection::map_variant_body1153::MapVariantBodyCapability1153),
    MapLava(g::caller_projection::map_lava1261::MapLavaCapability1261),
}
impl Cap {
    fn caller(&self) -> Result<&g::caller_projection::CallerCapability, String> {
        match self {
            Cap::Caller(c) => Ok(c),
            Cap::Foliage(f) => Ok(f.projection()),
            Cap::MapAlpha(m) => Ok(m.projection()),
            Cap::MapVariant(m) => Ok(m.projection()),
            Cap::MapLava(m) => Ok(m.projection()),
            Cap::MapStandard(_) => Err(transport("the map Standard capability exposes no caller projection")),
        }
    }
}
/// Rebuilds a capability. Outer `Err` = transport; inner = the seal's own refusal.
fn build(ctx: &Ctx, r: &CapRecipe) -> Result<R<Cap>, String> {
    use g::caller_projection as cp;
    macro_rules! src {
        ($w:expr) => {
            match ctx.source($w)? {
                Ok(s) => s,
                Err(e) => return Ok(Err(e)),
            }
        };
    }
    Ok(match r {
        CapRecipe::Caller { profile, fixtures, unit_t4 } => {
            let fixtures: Vec<cp::NullImageFixture> = conv("fixtures", fixtures)?;
            let t4 = match unit_t4 {
                Some(b) => Some(ctx.blob(b)?),
                None => None,
            };
            cp::CallerCapability::seal(profile, fixtures, t4.as_deref().map(|t4| cp::UnitConsumedBytes { t4 })).map(Cap::Caller).map_err(WireError::Message)
        }
        CapRecipe::Waves { images, buffers } => {
            cp::CallerCapability::seal_waves(conv("fixtures", images)?, conv("buffer fixtures", buffers)?).map(Cap::Caller).map_err(WireError::Message)
        }
        CapRecipe::Foliage { vs, ps, fixtures, t4, units } => {
            let (vs, ps) = (src!(vs), src!(ps));
            let fixtures: Vec<cp::NullImageFixture> = conv("fixtures", fixtures)?;
            let t4 = ctx.blob(t4)?;
            match units {
                None => cp::foliage_policy883::Foliage23Capability::seal(&vs, &ps, fixtures, &t4),
                Some((t5, t8)) => cp::foliage_policy883::Foliage23Capability::seal_unit_lights(&vs, &ps, fixtures, &t4, &ctx.blob(t5)?, &ctx.blob(t8)?),
            }
            .map(Cap::Foliage)
            .map_err(gerr)
        }
        CapRecipe::MapAlpha { vs, ps, null_images } => {
            cp::map_alpha_threshold1129::MapAlphaThresholdCapability1129::seal(&src!(vs), &src!(ps), null_images).map(Cap::MapAlpha).map_err(gerr)
        }
        CapRecipe::MapStandard { vs, ps, null_images } => {
            cp::standard_map1129::MapStandardCapability1129::seal(&src!(vs), &src!(ps), null_images).map(Cap::MapStandard).map_err(gerr)
        }
        CapRecipe::MapVariant { vs, ps, null_images } => {
            cp::map_variant_body1153::MapVariantBodyCapability1153::seal(&src!(vs), &src!(ps), null_images).map(Cap::MapVariant).map_err(gerr)
        }
        CapRecipe::MapLava { vs, ps, null_images } => {
            cp::map_lava1261::MapLavaCapability1261::seal(&src!(vs), &src!(ps), null_images).map(Cap::MapLava).map_err(gerr)
        }
        CapRecipe::Projection { of } => return build(ctx, of),
    })
}
fn seal_info(cap: &Cap) -> Result<SealInfo, String> {
    let ev = |c: &g::caller_projection::CallerCapability| -> Result<_, String> { Ok(Some(conv("evidence", c.evidence())?)) };
    Ok(match cap {
        Cap::Caller(c) => SealInfo { evidence: ev(c)?, ..Default::default() },
        Cap::Foliage(f) => SealInfo {
            evidence: ev(f.projection())?,
            consumed_t4: Some(f.consumed_t4()),
            unit_lights: f.unit_lights().map(|u| conv("unit lights", u)).transpose()?,
            receipt: Some(f.receipt()),
            receiver: Some(f.receiver()),
            null_images: None,
        },
        Cap::MapAlpha(m) => SealInfo { evidence: ev(m.projection())?, receipt: Some(m.receipt()), null_images: Some(m.null_images()), ..Default::default() },
        Cap::MapVariant(m) => SealInfo { evidence: ev(m.projection())?, receipt: Some(m.receipt()), null_images: Some(m.null_images()), ..Default::default() },
        Cap::MapLava(m) => SealInfo { evidence: ev(m.projection())?, receipt: Some(m.receipt()), null_images: Some(m.null_images()), ..Default::default() },
        Cap::MapStandard(_) => SealInfo::default(),
    })
}

fn identity() -> c::rpc::TranslatorIdentity {
    c::rpc::TranslatorIdentity {
        adapter_revision: g::ADAPTER_REVISION.into(),
        compute_revision: g::COMPUTE_REVISION.into(),
        consumed_build_identity: g::consumed_build_identity().map(str::to_owned),
        consumed_build_identity_sha256: g::consumed_build_identity_sha256().map(str::to_owned),
        require_consumed_build_identity: g::require_consumed_build_identity().map(str::to_owned).map_err(gerr),
        terrain_consumed_identity58: d2r_iridia_terrain53::require_consumed_identity58().map(|(a, b)| (a.to_owned(), b.to_owned())).map_err(str::to_owned),
        qualification_plan: format!("{:?}", g::unary_carrier::QualificationPlan::default()),
    }
}

fn ok(v: Value) -> Result<Response, String> {
    Ok(Response::Ok(v))
}
fn done<T: Serialize>(what: &str, r: R<T>) -> Result<Response, String> {
    match r {
        Ok(t) => ok(value(what, &t)?),
        Err(e) => Ok(Response::Err(e)),
    }
}
/// Runs one request. Outer `Err` = transport failure (not cached by the client).
fn dispatch(ctx: &Ctx, req: &Request) -> Result<Response, String> {
    macro_rules! src {
        ($w:expr) => {
            match ctx.source($w)? {
                Ok(s) => s,
                Err(e) => return Ok(Response::Err(e)),
            }
        };
    }
    macro_rules! cap {
        ($r:expr) => {
            match build(ctx, $r)? {
                Ok(c) => c,
                Err(e) => return Ok(Response::Err(e)),
            }
        };
    }
    match req {
        Request::Identity => ok(value("identity", &identity())?),
        Request::InspectSource { source } => done("inventory", g::inspect_source(&src!(source)).map_err(gerr)),
        Request::PreparePreciseSpirv { source } => match g::prepare_precise_spirv(&src!(source)) {
            Ok(p) => ok(value("precise spirv", &precise_wire(&p)?)?),
            Err(e) => Ok(Response::Err(gerr(e))),
        },
        Request::DiagnosticWgsl { source } => match g::diagnostic_wgsl(&src!(source)) {
            Ok(d) => {
                let w = c::DiagnosticWgsl { inventory: conv("inventory", &d.inventory)?, text: d.text.clone(), sha256: d.sha256.clone(), sampling_report: d.sampling_report.clone() };
                ok(value("diagnostic wgsl", &w)?)
            }
            Err(e) => Ok(Response::Err(gerr(e))),
        },
        Request::PrepareCompute { controlled, profile, source, caller, target } => {
            let s = src!(source);
            let (caller, target): (g::resource_contract::CallerContract, g::TargetContract) = (conv("caller", caller)?, conv("target", target)?);
            let r = if *controlled { g::prepare_controlled_compute104(profile, &s, &caller, &target) } else { g::prepare_compute(profile, &s, &caller, &target) };
            done("prepared shader", r.map_err(gerr))
        }
        Request::PrepareGraphics { controlled, profile, vs, ps, vertex, fragment, draw, target, projection } => {
            let (vs, ps) = (src!(vs), src!(ps));
            let cap = match projection {
                Some(r) => Some(cap!(r)),
                None => None,
            };
            let caller = match &cap {
                Some(c) => Some(c.caller()?),
                None => None,
            };
            let (vertex, fragment): (g::resource_contract::CallerContract, g::resource_contract::CallerContract) = (conv("vertex", vertex)?, conv("fragment", fragment)?);
            let (draw, target): (g::stage_link::DrawContract, g::TargetContract) = (conv("draw", draw)?, conv("target", target)?);
            let r = if *controlled {
                g::prepare_controlled_graphics104(profile, &vs, &ps, &vertex, &fragment, &draw, &target, caller)
            } else {
                g::prepare_graphics_pair(profile, &vs, &ps, &vertex, &fragment, &draw, &target, caller)
            };
            done("prepared graphics", r.map_err(gerr))
        }
        Request::PrepareStandardMap { vs, ps, vertex, physical_fragment, draw, target, cap } | Request::PrepareFoliage { vs, ps, vertex, physical_fragment, draw, target, cap } => {
            let (vs, ps) = (src!(vs), src!(ps));
            let cap = cap!(cap);
            let (vertex, fragment): (g::resource_contract::CallerContract, g::resource_contract::CallerContract) = (conv("vertex", vertex)?, conv("fragment", physical_fragment)?);
            let (draw, target): (g::stage_link::DrawContract, g::TargetContract) = (conv("draw", draw)?, conv("target", target)?);
            let r = match (req, &cap) {
                (Request::PrepareStandardMap { .. }, Cap::MapStandard(m)) => g::prepare_standard_map_graphics1129(&vs, &ps, &vertex, &fragment, &draw, &target, m),
                (Request::PrepareFoliage { .. }, Cap::Foliage(f)) => g::prepare_foliage_graphics883(&vs, &ps, &vertex, &fragment, &draw, &target, f),
                _ => return Err(transport(format!("{} with a capability of another kind", req.name()))),
            };
            done("prepared graphics", r.map_err(gerr))
        }
        Request::ProjectedCallerContract { source, caller, cap } => {
            let s = src!(source);
            let cap = cap!(cap);
            let caller: g::resource_contract::CallerContract = conv("caller", caller)?;
            done("caller contract", g::projected_caller_contract(&s, &caller, cap.caller()?).map_err(gerr))
        }
        Request::ValidateShader { artifact } => {
            let p: g::PreparedShader = serde_json::from_slice(&ctx.blob(artifact)?).map_err(|e| transport(format!("artifact: {e}")))?;
            done("validation", p.validate_artifact().map_err(gerr))
        }
        Request::ValidateGraphics { artifact } => {
            let p: g::PreparedGraphics = serde_json::from_slice(&ctx.blob(artifact)?).map_err(|e| transport(format!("artifact: {e}")))?;
            done("validation", p.validate_artifact().map_err(gerr))
        }
        Request::Seal { cap } => {
            let cap = cap!(cap);
            ok(value("seal", &seal_info(&cap)?)?)
        }
        Request::TerrainLoad { files } => done("terrain load", terrain_sources(ctx, files)?.map(|_| ())),
        Request::TerrainPrefix { files } => {
            let s = match terrain_sources(ctx, files)? {
                Ok(s) => s,
                Err(e) => return Ok(Response::Err(e)),
            };
            match t53::prepare_prefix(&s) {
                Ok(p) => ok(value("prefix", &t53_prefix_wire(&p)?)?),
                Err(e) => Ok(Response::Err(e.into())),
            }
        }
        Request::TerrainValidatePrefix { files, prefix } => {
            let s = match terrain_sources(ctx, files)? {
                Ok(s) => s,
                Err(e) => return Ok(Response::Err(e)),
            };
            let given: c::terrain_iridia::PrefixWire = serde_json::from_slice(&ctx.blob(prefix)?).map_err(|e| transport(format!("prefix: {e}")))?;
            let p = match t53::prepare_prefix(&s) {
                Ok(p) => p,
                Err(e) => return Ok(Response::Err(e.into())),
            };
            let mine = t53_prefix_wire(&p)?;
            if mine.prefix != given.prefix || mine.spirv != given.spirv {
                return Err(transport("the validated prefix differs from this translator's preparation of the same sources"));
            }
            done("validate prefix", t53::validate_prefix(&s, &p).map_err(WireError::from))
        }
        Request::TerrainWitness { files, witness } => {
            let s = match terrain_sources(ctx, files)? {
                Ok(s) => s,
                Err(e) => return Ok(Response::Err(e)),
            };
            done("witness", t53::terrain_scene::load_manifest(&s, Path::new(witness)).map(|_| ()).map_err(WireError::from))
        }
        Request::TerrainPixel { files, witness, shadow } => {
            let s = match terrain_sources(ctx, files)? {
                Ok(s) => s,
                Err(e) => return Ok(Response::Err(e)),
            };
            use t53::terrain_scene as ts;
            let r = match witness {
                Some(w) => ts::load_manifest(&s, Path::new(w)).and_then(|w| if *shadow { ts::shadow895::prepare_shadow_pixel(&s, &w) } else { ts::prepare_pixel(&s, &w) }),
                None => if *shadow { ts::shadow895::prepare_shadow_pixel_from_sources(&s) } else { ts::prepare_pixel_from_sources(&s) },
            };
            match r {
                Ok(p) => ok(value("pixel", &p)?),
                Err(e) => Ok(Response::Err(e.into())),
            }
        }
    }
}

fn terrain_sources(ctx: &Ctx, files: &BTreeMap<String, String>) -> Result<R<t53::Sources>, String> {
    let files = ctx.files(files)?;
    Ok(t53::Sources::load(|p| files.get(p).cloned().ok_or_else(|| d2r_shader::ShaderError::Io(std::io::Error::other(format!("PRETRANS1177: terrain source {p} not in the request"))))).map_err(WireError::from))
}
fn t53_prefix_wire(p: &t53::PreparedPrefix) -> Result<c::terrain_iridia::PrefixWire, String> {
    Ok(c::terrain_iridia::PrefixWire { prefix: value("prefix", p)?, spirv: p.spirv.clone() })
}

fn precise_wire(p: &g::precision_carrier::PreparedSpirv) -> Result<c::precision_carrier::PreparedSpirvWire, String> {
    Ok(c::precision_carrier::PreparedSpirvWire {
        words: p.words().to_vec(),
        digest: p.payload_sha256().to_owned(),
        receipt: c::precision_carrier::PrecisionReceipt {
            identity: p.receipt().identity().to_string(),
            records: p.receipt().records().iter().map(|r| format!("{r:?}")).collect(),
        },
        r2_dropped_integer_no_contraction: p.emission_report648().r2_865.as_ref().map(|r| r.dropped_integer_no_contraction.clone()),
        imported_debug_sha256: c::sha256(format!("{:?}", p.imported_module()).as_bytes()),
        validate_payload: p.validate_payload(p.words()),
        controlled: match g::payload104::ShaderPayload104::controlled(p) {
            Ok(x) => Ok(conv("controlled payload", &x)?),
            Err(e) => Err(gerr(e).into_error()),
        },
    })
}

// --------------------------------------------------------------------------------- commands --

fn own_sha256() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(transport)?;
    Ok(c::sha256(&std::fs::read(&exe).map_err(|e| transport(format!("{}: {e}", exe.display())))?))
}

/// `D2R_VERSION_TOLERANT` as both readers in the closure parse it (d2r-shader compiler.rs and
/// d2r-shader-iridia source.rs).
fn version_tolerant1177() -> bool {
    std::env::var_os("D2R_VERSION_TOLERANT").is_some_and(|v| !v.is_empty() && v != "0")
}

/// serde_json behaviour that a feature unification could change (key order, number text).
fn serde_probe1177() -> String {
    let mut m = serde_json::Map::new();
    m.insert("b".into(), Value::from(1));
    m.insert("a".into(), serde_json::from_str::<Value>("1.0000000000000002").unwrap_or(Value::Null));
    serde_json::to_string(&m).unwrap_or_default()
}

/// Everything the cache id is made of. The build part comes from build.rs; the run-time
/// part is what this process would answer differently on another PC or with another environment:
/// its identity answer (consumed build identity and qualification), D2R_VERSION_TOLERANT and a
/// serde_json probe. This build reads no other local files at run time.
fn cache_inputs1177(tolerant: bool) -> Value {
    serde_json::json!({
        "schema": "dxbc-shader-translator-id-v1",
        "build": built1177::TRANSLATION_BUILD1177,
        "identity": identity(),
        "version_tolerant": tolerant,
        "serde_json_probe": serde_probe1177(),
    })
}
fn cache_id_of(inputs: &Value) -> Result<String, String> {
    Ok(c::sha256(&rpc::canonical_json(inputs).map_err(transport)?))
}
/// The id that names this translator's cache folder (`t-<16 hex>`).
fn cache_id() -> Result<String, String> {
    cache_id_of(&cache_inputs1177(version_tolerant1177()))
}

/// One request file: checks its key and folder, runs it and writes the result.
fn run(root: &Path, req_path: &Path, own: &str) -> Result<(), String> {
    let bytes = std::fs::read(req_path).map_err(|e| transport(format!("{}: {e}", req_path.display())))?;
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|e| transport(format!("{}: {e}", req_path.display())))?;
    if envelope.schema != rpc::SCHEMA {
        return Err(transport(format!("request schema {} (this translator: {})", envelope.schema, rpc::SCHEMA)));
    }
    let (canonical, key) = rpc::request_key(&envelope).map_err(transport)?;
    let dir = rpc::translator_dir(root, own);
    let (expected_req, res) = rpc::request_paths(&dir, &key);
    if canonical != bytes || std::fs::canonicalize(&expected_req).ok() != std::fs::canonicalize(req_path).ok() {
        return Err(transport(format!("{} is not the canonical request file of this translator build ({})", req_path.display(), expected_req.display())));
    }
    let ctx = Ctx { root: root.to_owned() };
    let response = dispatch(&ctx, &envelope.request)?;
    let out = serde_json::to_vec(&response).map_err(transport)?;
    rpc::write_atomic(&res, &out).map_err(|e| transport(format!("{}: {e}", res.display())))
}

/// Every logged request of this PC without a result for this translator build, on a few worker
/// threads. A request another process holds the lock of (the viewer's own call) is skipped.
/// The viewer holds the write end of this process's standard input. When it ends
/// (normal exit, crash or a stop from the task manager), the read returns its end and this process
/// exits at once. Request locks are delete-on-close handles, so none stays behind; a result being
/// written is a temporary file that is never renamed into place, so no partial result exists.
fn stop_when_stdin_closes() {
    std::thread::spawn(|| {
        use std::io::Read;
        let mut sink = [0u8; 64];
        let mut stdin = std::io::stdin().lock();
        while matches!(stdin.read(&mut sink), Ok(n) if n > 0) {}
        eprintln!("PRETRANS1177 warm: the viewer ended; warm-up stopped");
        std::process::exit(0);
    });
}
fn warm(root: &Path, own: &str) -> Result<(usize, usize), String> {
    let log = std::fs::read_to_string(root.join("requests.log")).unwrap_or_default();
    let dir = rpc::translator_dir(root, own);
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = Vec::new();
    for line in log.lines() {
        // `t-<build>/<kk>/<key>`: the key names the request content; this build has its own folder.
        let Some(key) = line.rsplit('/').next().filter(|k| k.len() == 64 && k.bytes().all(|b| b.is_ascii_hexdigit())) else { continue };
        if seen.insert(key.to_owned()) && !rpc::request_paths(&dir, key).1.exists() {
            pending.push((key.to_owned(), root.join(line).with_extension("req.json")));
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let (done_n, failed) = (std::sync::atomic::AtomicUsize::new(0), std::sync::atomic::AtomicUsize::new(0));
    let workers = std::thread::available_parallelism().map(|n| (n.get() / 4).clamp(1, 4)).unwrap_or(1);
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| {
                while let Some((key, old)) = pending.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)) {
                    let (req, res) = rpc::request_paths(&dir, key);
                    let Some(_lock) = rpc::Lock::try_take(&res) else { continue };
                    if !req.exists() {
                        match std::fs::read(old) {
                            Ok(b) if rpc::write_atomic(&req, &b).is_ok() => {}
                            _ => continue,
                        }
                    }
                    match run(root, &req, own) {
                        Ok(()) => done_n.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                        Err(e) => {
                            eprintln!("{e}");
                            failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                        }
                    };
                }
            });
        }
    });
    Ok((done_n.into_inner(), failed.into_inner()))
}

/// Every cached result of this translator build is translated again in this process and
/// compared byte for byte with the stored result (`<key>.res.json`). Returns (equal, differ).
fn verify(root: &Path, own: &str) -> Result<(usize, usize), String> {
    let dir = rpc::translator_dir(root, own);
    let mut results = Vec::new();
    for kk in std::fs::read_dir(&dir).map_err(|e| transport(format!("{}: {e}", dir.display())))?.flatten() {
        for f in std::fs::read_dir(kk.path()).into_iter().flatten().flatten() {
            let name = f.file_name().to_string_lossy().into_owned();
            if let Some(key) = name.strip_suffix(".res.json") {
                results.push(key.to_owned());
            }
        }
    }
    results.sort();
    let ctx = Ctx { root: root.to_owned() };
    let (mut equal, mut differ) = (0, 0);
    for key in &results {
        let (req, res) = rpc::request_paths(&dir, key);
        let stored = std::fs::read(&res).map_err(|e| transport(format!("{}: {e}", res.display())))?;
        let bytes = std::fs::read(&req).map_err(|e| transport(format!("{}: {e}", req.display())))?;
        let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|e| transport(format!("{}: {e}", req.display())))?;
        let again = serde_json::to_vec(&dispatch(&ctx, &envelope.request)?).map_err(transport)?;
        if again == stored {
            equal += 1;
        } else {
            differ += 1;
            eprintln!("PRETRANS1177 VERIFY DIFFER {} {key} ({} vs {} bytes) {}", envelope.request.name(), stored.len(), again.len(), String::from_utf8_lossy(&again[..again.len().min(300)]));
        }
    }
    Ok((equal, differ))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1).map(String::as_str) {
        Some("run") if args.len() == 4 => cache_id().and_then(|own| run(Path::new(&args[2]), Path::new(&args[3]), &own)).map(|()| 0),
        // The viewer reads its cache folder from this line (`rpc::CacheIdLine`).
        Some("cache-id") if args.len() == 2 => cache_id().map(|id| {
            println!("{}", serde_json::json!({"cache_id": id, "folder": format!("t-{}", &id[..16])}));
            0
        }),
        Some("identity") => own_sha256().and_then(|own| {
            let inputs = cache_inputs1177(version_tolerant1177());
            let id = cache_id_of(&inputs)?;
            println!(
                "{}",
                serde_json::json!({"cache_id": id, "folder": format!("t-{}", &id[..16]), "cache_inputs": inputs,
                    "build_inputs": built1177::TRANSLATION_BUILD_LINES1177, "translator_sha256": own, "identity": identity()})
            );
            Ok(0)
        }),
        Some("warm") if args.len() == 3 || (args.len() == 4 && args[3] == rpc::WARM_PARENT_FLAG) => cache_id().and_then(|own| {
            if args.len() == 4 {
                stop_when_stdin_closes();
            }
            warm(Path::new(&args[2]), &own)
        }).map(|(n, f)| {
            eprintln!("PRETRANS1177 warm: {n} translated, {f} failed");
            i32::from(f > 0)
        }),
        Some("verify") if args.len() == 3 => cache_id().and_then(|own| verify(Path::new(&args[2]), &own)).map(|(e, d)| {
            println!("PRETRANS1177 verify: {e} equal, {d} differ");
            i32::from(d > 0)
        }),
        _ => Err("usage: d2r-shader-translator run <cache root> <request.req.json> | cache-id | identity | warm <cache root> [--until-stdin-closes] | verify <cache root>".into()),
    };
    match code {
        Ok(c) => std::process::exit(c),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    }
}

