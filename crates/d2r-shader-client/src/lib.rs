//! GPL-free client of the original-shader translator.
//!
//! The viewer links this crate under the names `d2r_shader_iridia` and `d2r_iridia_terrain53`
//! (the host renderer) unless the in-process development features are on. It exposes the module
//! paths and names the viewer uses from those GPL crates:
//!
//! - data definitions (contracts, sources, prepared artifacts, payloads) with the same serde form,
//!   copied from the project's own GPL-labelled crate (structs and accessors, no translation code);
//! - every function with translation or validation logic as a cached call into the separate program
//!   `d2r-shader-translator` ([`rpc`]). The translator runs the unchanged GPL functions, so the
//!   viewer receives the bytes the in-process build produces.
//!
//! A missing translator or failed run is an explicit `PRETRANS1177 ERROR`; there is no fallback.

mod cache;
pub mod caller_projection;
pub mod payload104;
pub mod precision_carrier;
mod prepare;
pub mod resource_contract;
pub mod rpc;
mod source;
pub mod stage_link;
mod terrain;

pub use cache::{CacheIdentity, GRAPHICS_SCHEMA, PreparedGraphics, PreparedShader, canonical_metadata_sha256};
pub use prepare::{ComputeLimits, ICE_PROFILE, PAIR_SCHEMA, TargetContract};
pub use source::{SourceIdentity, SourceShader, Stage, extract_original, sha256, tolerant_dxbc};
pub use terrain::{require_consumed_identity58, terrain_iridia};

use caller_projection::CallerCapability;
use resource_contract::CallerContract;
use rpc::{Request, call_e, src};
use serde::{Deserialize, Serialize};
use stage_link::DrawContract;
use std::{fmt, sync::Arc};

pub const ADAPTER_REVISION: &str = "d2r-iridia865d-r3r4-v1-precision32";
pub const COMPUTE_REVISION: &str = "d2r-iridia50-explicit-compute-target-v2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid(String),
    Identity(String),
    Unsupported(String),
    Validation(String),
    Io(String),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Invalid(e.to_string())
    }
}
/// The wire form (the translator's `Result` of a function that returns this crate's error).
impl Serialize for Error {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        rpc::WireError::from(self.clone()).serialize(s)
    }
}
impl<'de> Deserialize<'de> for Error {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        Ok(rpc::WireError::deserialize(d)?.into_error())
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SourceInventory {
    pub source: SourceIdentity,
    pub interface: serde_json::Value,
    pub lifter: String,
    pub lifter_version: u32,
    pub program_version: u32,
    pub notes: Vec<String>,
}
pub fn inspect_source(source: &SourceShader) -> Result<SourceInventory> {
    call_e(Request::InspectSource { source: src(source)? })
}

pub fn prepare_precise_spirv(source: &SourceShader) -> Result<precision_carrier::PreparedSpirv> {
    let wire: precision_carrier::PreparedSpirvWire = call_e(Request::PreparePreciseSpirv { source: src(source)? })?;
    precision_carrier::PreparedSpirv::from_wire(wire)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagnosticWgsl {
    pub inventory: SourceInventory,
    pub text: Arc<str>,
    pub sha256: String,
    pub sampling_report: serde_json::Value,
}
pub fn diagnostic_wgsl(source: &SourceShader) -> Result<DiagnosticWgsl> {
    call_e(Request::DiagnosticWgsl { source: src(source)? })
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
fn identity_text(pick: fn(&rpc::TranslatorIdentity) -> Option<String>) -> Option<&'static str> {
    rpc::identity().ok().and_then(|id| pick(&id)).map(|s| leak(&s))
}
/// The translator's consumed-build identity (its GPL crates' build); `None` also when the
/// translator cannot run (the `PRETRANS1177 ERROR` line is printed).
pub fn consumed_build_identity() -> Option<&'static str> {
    static V: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    *V.get_or_init(|| identity_text(|i| i.consumed_build_identity.clone()))
}
pub fn consumed_build_identity_sha256() -> Option<&'static str> {
    static V: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
    *V.get_or_init(|| identity_text(|i| i.consumed_build_identity_sha256.clone()))
}
pub fn require_consumed_build_identity() -> Result<&'static str> {
    static V: std::sync::OnceLock<Result<&'static str>> = std::sync::OnceLock::new();
    V.get_or_init(|| match rpc::identity()?.require_consumed_build_identity.clone() {
        Ok(s) => Ok(leak(&s)),
        Err(e) => Err(e.into_error()),
    })
    .clone()
}

pub fn prepare_compute(profile: &str, source: &SourceShader, caller: &CallerContract, target: &TargetContract) -> Result<PreparedShader> {
    compute(false, profile, source, caller, target)
}
pub fn prepare_controlled_compute104(profile: &str, source: &SourceShader, caller: &CallerContract, target: &TargetContract) -> Result<PreparedShader> {
    compute(true, profile, source, caller, target)
}
fn compute(controlled: bool, profile: &str, source: &SourceShader, caller: &CallerContract, target: &TargetContract) -> Result<PreparedShader> {
    call_e(Request::PrepareCompute { controlled, profile: profile.into(), source: src(source)?, caller: caller.clone(), target: target.clone() })
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_graphics_pair(
    profile: &str,
    vs: &SourceShader,
    ps: &SourceShader,
    vertex: &CallerContract,
    fragment: &CallerContract,
    draw: &DrawContract,
    target: &TargetContract,
    fragment_projection: Option<&CallerCapability>,
) -> Result<PreparedGraphics> {
    graphics(false, profile, vs, ps, vertex, fragment, draw, target, fragment_projection)
}
#[allow(clippy::too_many_arguments)]
pub fn prepare_controlled_graphics104(
    profile: &str,
    vs: &SourceShader,
    ps: &SourceShader,
    vertex: &CallerContract,
    fragment: &CallerContract,
    draw: &DrawContract,
    target: &TargetContract,
    fragment_projection: Option<&CallerCapability>,
) -> Result<PreparedGraphics> {
    graphics(true, profile, vs, ps, vertex, fragment, draw, target, fragment_projection)
}
#[allow(clippy::too_many_arguments)]
fn graphics(
    controlled: bool,
    profile: &str,
    vs: &SourceShader,
    ps: &SourceShader,
    vertex: &CallerContract,
    fragment: &CallerContract,
    draw: &DrawContract,
    target: &TargetContract,
    projection: Option<&CallerCapability>,
) -> Result<PreparedGraphics> {
    call_e(Request::PrepareGraphics {
        controlled,
        profile: profile.into(),
        vs: src(vs)?,
        ps: src(ps)?,
        vertex: vertex.clone(),
        fragment: fragment.clone(),
        draw: draw.clone(),
        target: target.clone(),
        projection: projection.map(|c| c.recipe().clone()),
    })
}
pub fn prepare_standard_map_graphics1129(
    vs: &SourceShader,
    ps: &SourceShader,
    vertex: &CallerContract,
    physical_fragment: &CallerContract,
    draw: &DrawContract,
    target: &TargetContract,
    cap: &caller_projection::standard_map1129::MapStandardCapability1129,
) -> Result<PreparedGraphics> {
    call_e(Request::PrepareStandardMap {
        vs: src(vs)?,
        ps: src(ps)?,
        vertex: vertex.clone(),
        physical_fragment: physical_fragment.clone(),
        draw: draw.clone(),
        target: target.clone(),
        cap: cap.recipe.clone(),
    })
}
#[allow(clippy::too_many_arguments)]
pub fn prepare_foliage_graphics883(
    vs: &SourceShader,
    ps: &SourceShader,
    vertex: &CallerContract,
    physical_fragment: &CallerContract,
    draw: &DrawContract,
    target: &TargetContract,
    cap: &caller_projection::foliage_policy883::Foliage23Capability,
) -> Result<PreparedGraphics> {
    call_e(Request::PrepareFoliage {
        vs: src(vs)?,
        ps: src(ps)?,
        vertex: vertex.clone(),
        physical_fragment: physical_fragment.clone(),
        draw: draw.clone(),
        target: target.clone(),
        cap: cap.recipe.clone(),
    })
}
pub fn projected_caller_contract(source: &SourceShader, caller: &CallerContract, cap: &CallerCapability) -> Result<CallerContract> {
    call_e(Request::ProjectedCallerContract { source: src(source)?, caller: caller.clone(), cap: cap.recipe().clone() })
}
