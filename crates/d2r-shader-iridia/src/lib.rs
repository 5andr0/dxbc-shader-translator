//! Source-authenticated DXBC preparation using the published Iridia API.
//!
//! The forked Naga module stays private: consumers borrow immutable WGSL or
//! controlled SPIR-V and metadata, validated with the renderer's stock Naga. No platform converter,
//! external process or runtime script participates in this crate.
mod build_identity;
mod source;
pub use build_identity::{
    consumed_build_identity, consumed_build_identity_sha256, require_consumed_build_identity,
};
pub use source::{SourceIdentity, SourceShader, Stage, extract_original, sha256};
mod cache;
pub use cache::{
    CacheIdentity, PreparedShader, SharedPreparedCache, canonical_metadata_sha256, load_cache,
    store_cache,
};
pub mod caller_projection;
/// The background guard types have no values in this program, so every
/// background branch of the shared preparation code is unreachable.
mod background_guard438 {
    use crate::{Result, SourceShader};
    pub enum BackgroundGuardCapability438 {}
    pub enum BackgroundOwnedInputs438 {}
    pub const BACKGROUND_REVISION438: &str = "retained177-background-three-draw-guards-v1";
    impl BackgroundGuardCapability438 {
        pub fn evidence(&self) -> serde_json::Value { match *self {} }
        pub(crate) fn recheck(&self, _: &std::sync::Arc<BackgroundOwnedInputs438>, _: &SourceShader, _: &SourceShader) -> Result<()> { match *self {} }
    }
    pub(crate) fn apply(_: &mut iridia::Program, cap: &BackgroundGuardCapability438) -> Result<crate::caller_projection::CallerProjectionReport> { match *cap {} }
}
mod prepare;
pub mod resource_contract;
pub mod stage_link;
pub use prepare::{
    ComputeLimits, ICE_PROFILE, PAIR_SCHEMA, PreparedPair, TargetContract, prepare_ice_pair,
};
mod sample_lod;
pub mod sample_emit648;
pub mod sample_import648;
mod typed_null_note;

use serde::{Deserialize, Serialize};
use std::{fmt, sync::Arc};

/// Adapter revision, distinct from every retained vkd3d conversion identity.
pub const ADAPTER_REVISION: &str = "d2r-iridia865d-r3r4-v1-precision32";
mod graphics;
mod validation;
mod spirv_interface883;
pub use graphics::{
    GRAPHICS_SCHEMA, PreparedGraphics, prepare_graphics_pair, projected_caller_contract,
};
mod compute;
pub use compute::{COMPUTE_REVISION, prepare_compute};

/// A named failure; unsupported behavior must never fall back silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Invalid extent, malformed input, or corrupted artifact.
    Invalid(String),
    /// Exact source identity or caller contract did not match.
    Identity(String),
    /// Source operation or caller contract is not implemented.
    Unsupported(String),
    /// Iridia, WGSL or stock Naga refused the prepared module.
    Validation(String),
    /// Bounded I/O failed.
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
pub type Result<T> = std::result::Result<T, Error>;

/// Owned original declaration/interface inventory, without an IR type escape.
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

/// Lift only to report source declarations. This is not a rendering admission.
pub fn inspect_source(source: &SourceShader) -> Result<SourceInventory> {
    let program = lift(source)?;
    Ok(SourceInventory {
        source: source.identity().clone(),
        interface: serde_json::to_value(&program.interface)?,
        lifter: program.provenance.lifter,
        lifter_version: program.provenance.lifter_version,
        program_version: iridia::PROGRAM_VERSION,
        notes: program.notes,
    })
}

/// The precise carrier for a precision-bearing source: lift, seal and prepare the SPIR-V
/// payload with every precision join (`precision_carrier::prepare_spirv`). Diagnostic CPU entry.
pub fn prepare_precise_spirv(source: &SourceShader) -> Result<precision_carrier::PreparedSpirv> {
    let program = lift(source)?;
    precision_carrier::prepare_spirv(&program, &Default::default()).map_err(Error::Unsupported)
}

fn lift(source: &SourceShader) -> Result<iridia::Program> {
    let program = iridia_dxbc::lift(source.bytes())
        .map_err(|e| Error::Unsupported(format!("DXBC lift: {e}")))?;
    let stage = match program.stage {
        iridia::Stage::Vertex => Stage::Vertex,
        iridia::Stage::Pixel => Stage::Fragment,
        iridia::Stage::Compute => Stage::Compute,
        iridia::Stage::Hull => Stage::Hull,
        iridia::Stage::Domain => Stage::Domain,
        _ => return Err(Error::Unsupported(format!("stage {:?}", program.stage))),
    };
    if stage != source.identity().stage {
        return Err(Error::Identity(
            "source stage differs from authenticated profile".into(),
        ));
    }
    if program.provenance.source_hash.as_deref()
        != Some(format!("sha256:{}", source.identity().dxbc_sha256).as_str())
    {
        return Err(Error::Identity(
            "lifter provenance differs from original bytes".into(),
        ));
    }
    Ok(program)
}

/// Validate WGSL with the same stock Naga release used by wgpu/Bevy.
pub fn validate_stock_wgsl(text: &str) -> Result<()> {
    if text.len() > 16 * 1024 * 1024 {
        return Err(Error::Invalid("WGSL extent".into()));
    }
    let module = naga::front::wgsl::parse_str(text)
        .map_err(|e| Error::Validation(e.emit_to_string(text)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| Error::Validation(format!("stock Naga: {e:?}")))?;
    Ok(())
}

/// A validated source program for diagnostics only. Resource contracts and stage
/// linking are required before any product/GPU admission. No renderer consumes
/// this type as a PreparedShader.
#[derive(Clone, Debug)]
pub struct DiagnosticWgsl {
    pub inventory: SourceInventory,
    pub text: Arc<str>,
    pub sha256: String,
    pub sampling_report: serde_json::Value,
}
pub fn diagnostic_wgsl(source: &SourceShader) -> Result<DiagnosticWgsl> {
    let mut program = lift(source)?;
    // Precise stages are consumed through the SPIR-V/HLSL payload. WGSL has no
    // arithmetic-control representation, so a precision-bearing program is reported, never lowered
    // with its controls dropped.
    if program.precision.is_precision_bearing() {
        return Err(Error::Unsupported(format!(
            "NotRepresentableInWgsl: precise program (receipt {}); consume the SPIR-V/HLSL payload (prepare_precise_spirv)",
            iridia::precision::RECEIPT_REVISION
        )));
    }
    let sampling_report = apply_sampling(&mut program)?;
    program
        .validate()
        .map_err(|e| Error::Validation(format!("Iridia: {e:?}")))?;
    let text = iridia::emit::wgsl(&program)
        .map_err(|e| Error::Unsupported(format!("WGSL emission: {e}")))?;
    validate_stock_wgsl(&text)?;
    Ok(DiagnosticWgsl {
        inventory: inspect_source(source)?,
        sha256: sha256(text.as_bytes()),
        text: text.into(),
        sampling_report,
    })
}

fn apply_sampling(program: &mut iridia::Program) -> Result<serde_json::Value> {
    let r = sample_lod::repair_exact_lod(program)
        .map_err(|e| Error::Unsupported(format!("exact source LOD: {e}")))?;
    Ok(
        serde_json::json!({"revision":sample_lod::SAMPLE_LOD_REVISION,
        "integer_depth_exact_preserved":r.integer_depth_exact_preserved,
        "ordinary_exact_preserved":r.ordinary_exact_preserved,
        "repaired":r.repaired.iter().map(|s|serde_json::json!({"function":s.function,"expression":s.expression,"image":s.image,"old_lod_expression":s.old_lod_expression,"new_lod_expression":s.new_lod_expression,"original_f32_bits":s.original_f32_bits})).collect::<Vec<_>>() }),
    )
}

mod raw_carrier582;
mod bit_carrier575;
mod selection584;
pub mod precision_carrier;

pub mod exact_move_carrier;

pub mod unary_carrier;

pub mod round_ni_carrier849;
pub mod compare_carrier865;
pub mod frac_carrier865d;
pub mod max_carrier865e;
pub mod tolerance_carrier865c;
pub mod sat_carrier865d;
pub mod modifier_carrier849b;

pub mod payload104;

pub use compute::prepare_controlled_compute104;

pub use graphics::prepare_controlled_graphics104;

pub mod authored122;

pub use graphics::prepare_standard_graphics668;
pub use graphics::prepare_standard_map_graphics1129;

pub use graphics::prepare_background_graphics677;

pub use graphics::prepare_foliage_graphics883;

#[path="../../d2r-shader/src/authored_idle830/domain.rs"]pub mod receiver_domain830;
pub mod authored_light830;
