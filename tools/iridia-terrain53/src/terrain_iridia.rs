//! Original Terrain source-directed composition, with independent Iridia facts.
//! No platform DXBC compiler is loaded, called, selected or used as a fallback.
//! The existing authored 402-node composer is itself pure Rust; its final SPIR-V
//! to WGSL helper is stock Naga, despite the historical helper module's name.
use d2r_shader::{
    ShaderError, sha256,
    terrain::{self, OriginalTerrainStage as Stage, TerrainSources},
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
#[path = "passes/resource_contract.rs"]
mod resource_contract;
#[path = "passes/sample_lod.rs"]
mod sample_lod;
#[path = "terrain_scene.rs"]
pub mod terrain_scene;

pub const REVISION: &str = "terrain53-source-directed-402-iridia-provenance-v2";
fn bad(s: impl Into<String>) -> ShaderError {
    ShaderError::Unsupported(format!("Terrain53: {}", s.into()))
}

/// Owns only original resources; no serialized summary/default constructor.
/// Program selection (pinned slice, or the version-tolerant search under
/// `D2R_VERSION_TOLERANT=1`) is `d2r_shader::terrain::original_code`.
pub struct Sources {
    canonical: TerrainSources,
}
impl Sources {
    pub fn load(
        mut read: impl FnMut(&str) -> Result<Vec<u8>, ShaderError>,
    ) -> Result<Self, ShaderError> {
        let mut resources = BTreeMap::new();
        for stage in Stage::ALL {
            let p = terrain::profile(stage);
            resources.insert(p.path.clone(), read(&p.path)?);
        }
        let canonical = TerrainSources::load(|path| {
            resources
                .get(path)
                .cloned()
                .ok_or_else(|| bad("missing source"))
        })?;
        // Original HS instruction/operand gate proves all FOUR factors rawf32 one.
        let typed = terrain::inspect(&canonical)?;
        if typed.hull.factor_bits != [0x3f800000; 4]
            || typed.hull.input_control_points != 3
            || typed.hull.output_control_points != 3
            || typed.hull.partitioning != "fractional_odd"
            || typed.hull.output_primitive != "triangle_cw"
        {
            return Err(bad(
                "only exact source fixed-factor1 triangle fractional_odd supported",
            ));
        }
        Ok(Self { canonical })
    }
    fn code(&self, stage: Stage) -> Result<&[u8], ShaderError> {
        Ok(self.canonical.original(stage))
    }
}

#[derive(Debug, Serialize)]
pub struct LiftReceipt {
    pub original: terrain::StageProfile,
    pub iridia_version: &'static str,
    pub lifter_version: u32,
    pub provenance: Value,
    pub interface: Value,
    pub declared_functions: String,
    pub module_entry_points: usize,
    pub module_functions: usize,
    pub bound_constant_buffer_views: Vec<(u32, u32)>,
    pub validated: bool,
    pub notes: Vec<String>,
    pub iridia_function_body_used_as_execution_replacement: bool,
}

fn lift_prefix(s: &Sources, stage: Stage) -> Result<LiftReceipt, ShaderError> {
    let bytes = s.code(stage)?;
    let initial = iridia_dxbc::lift(bytes).map_err(|e| bad(e.to_string()))?;
    let expected = match stage {
        Stage::Vertex => iridia::Stage::Vertex,
        Stage::Hull => iridia::Stage::Hull,
        Stage::Domain => iridia::Stage::Domain,
        Stage::Fragment => return Err(bad("fragment uses separate resource/sampling adapter")),
    };
    if initial.stage != expected
        || initial.provenance.source_hash.as_deref() != Some(iridia::source_hash(bytes).as_str())
    {
        return Err(bad("Iridia source/stage provenance mismatch"));
    }
    let mut bound = iridia::BoundSizes::new();
    let mut cb = vec![];
    for r in &initial.interface.resources {
        let Some(b) = r.provenance.d3d else {
            return Err(bad("missing original binding provenance"));
        };
        if b.class != iridia::D3dClass::ConstantBuffer
            || b.space != 0
            || r.count != 1
            || !matches!(&r.kind, iridia::ResourceKind::ConstantBuffer { .. })
        {
            return Err(bad("unknown prefix source resource"));
        }
        cb.push(b.register);
        bound.set(r.id, 1536);
    }
    cb.sort();
    if (stage == Stage::Vertex && cb != [0, 3]) || (stage != Stage::Vertex && !cb.is_empty()) {
        return Err(bad("exact original VSb0/b3; HS/DS resource-free closure"));
    }
    let p = iridia_dxbc::lift_with(bytes, &bound).map_err(|e| bad(e.to_string()))?;
    p.validate()
        .map_err(|e| bad(format!("original prefix IR validation: {e:?}")))?;
    match (&p.interface.facts, stage) {
        (iridia::StageFacts::Hull(f), Stage::Hull)
            if f.input_control_points == 3
                && f.output_control_points == 3
                && f.domain == iridia::TessDomain::Triangle
                && f.partitioning == iridia::Partitioning::FractionalOdd
                && f.topology == iridia::TessTopology::TriangleCw => {}
        (iridia::StageFacts::Domain(f), Stage::Domain)
            if f.input_control_points == 3 && f.domain == iridia::TessDomain::Triangle => {}
        (_, Stage::Vertex) => {}
        _ => {
            return Err(bad(
                "Iridia stage declarations disagree with original source contract",
            ));
        }
    }
    // No HS/DS stage relabel or entrypoint synthesis from lifted stage functions.
    if stage != Stage::Vertex && !p.module.entry_points.is_empty() {
        return Err(bad(
            "HS/DS must remain functions, not invented WGSL entries",
        ));
    }
    Ok(LiftReceipt {
        original: terrain::profile(stage),
        iridia_version: "0.5.0",
        lifter_version: iridia_dxbc::LIFTER_VERSION,
        provenance: json!({"format":format!("{:?}",p.provenance.format),"target":p.provenance.target,
            "source_hash":p.provenance.source_hash,"lifter":p.provenance.lifter,
            "lifter_version":p.provenance.lifter_version}),
        interface: serde_json::to_value(&p.interface).map_err(|e| bad(e.to_string()))?,
        declared_functions: format!("{:?}", iridia::stage_functions(&p.interface).map_err(bad)?),
        module_entry_points: p.module.entry_points.len(),
        module_functions: p.module.functions.len(),
        bound_constant_buffer_views: cb.into_iter().map(|register| (register, 1536)).collect(),
        validated: true,
        notes: p.notes,
        iridia_function_body_used_as_execution_replacement: false,
    })
}

/// Owned public bridge: fork-Naga and Iridia module handles stay private.
#[derive(Serialize)]
pub struct PreparedPrefix {
    pub revision: &'static str,
    pub preparation_key: String,
    pub compiler_identity: Value,
    pub lifts: Vec<LiftReceipt>,
    pub canonical: terrain::TerrainProgram,
    #[serde(skip)]
    pub spirv: Vec<u8>,
    pub original_source_to_402_projection: bool,
    pub native_compiler_called: bool,
    pub native_compiler_fallback_available: bool,
    pub general_tessellation_supported: bool,
    pub automatic_native_domain_producer: bool,
    pub numerical_or_raster_acceptance: bool,
}
pub fn prepare_prefix(s: &Sources) -> Result<PreparedPrefix, ShaderError> {
    let lifts = [Stage::Vertex, Stage::Hull, Stage::Domain]
        .into_iter()
        .map(|stage| lift_prefix(s, stage))
        .collect::<Result<Vec<_>, _>>()?;
    // This is the SAME already-reviewed mechanical source-token→402-node IR
    // composer, not a shader copied by hand and not Iridia's stage relabelling.
    let (canonical, spirv) = terrain::compose(&s.canonical)?;
    if canonical.graph.nodes.len() != 402
        || canonical.projection.len() != 402
        || canonical.graph.outputs.iter().map(Vec::len).sum::<usize>() != 21
        || sha256(&spirv) != canonical.spirv_sha256
        || sha256(canonical.wgsl.as_bytes()) != canonical.wgsl_sha256
    {
        return Err(bad("complete original402-node projection/output identity"));
    }
    let m = naga::front::wgsl::parse_str(&canonical.wgsl).map_err(|e| bad(e.to_string()))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&m)
    .map_err(|e| bad(e.to_string()))?;
    let compiler_identity = json!({"adapter_revision":REVISION,"iridia":"0.5.0",
        "dxbc_lifter_version":iridia_dxbc::LIFTER_VERSION,"stock_naga":"30.0.1",
        "locked_package_graph_sha256":sha256(include_bytes!("../Cargo.lock")),
        "sampling51_revision":sample_lod::SAMPLE_LOD_REVISION,
        "resource52_revision":resource_contract::RESOURCE_REVISION,
        "originals":s.canonical.identities(),"composition_revision":canonical.revision,
        "geometry":canonical.geometry,"canonical_spirv_sha256":canonical.spirv_sha256,
        "canonical_wgsl_sha256":canonical.wgsl_sha256,
        "route":"authenticated_original_tokens_to_existing_typed402_to_stock_naga",
        "old_native_compiler_identity_reused":false,
        "domain_policy":"explicit_caller_raw_domain_bits; measured-device/fixture capability separate"});
    let preparation_key =
        sha256(&serde_json::to_vec(&compiler_identity).map_err(|e| bad(e.to_string()))?);
    Ok(PreparedPrefix {
        revision: REVISION,
        preparation_key,
        compiler_identity,
        lifts,
        canonical,
        spirv,
        original_source_to_402_projection: true,
        native_compiler_called: false,
        native_compiler_fallback_available: false,
        general_tessellation_supported: false,
        automatic_native_domain_producer: false,
        numerical_or_raster_acceptance: false,
    })
}

/// Source-bound admission, not a self-hash-only cache check. No compiler fallback.
pub fn validate_prefix(s: &Sources, p: &PreparedPrefix) -> Result<(), ShaderError> {
    let expected = prepare_prefix(s)?;
    if p.spirv != expected.spirv
        || serde_json::to_vec(p).map_err(|e| bad(e.to_string()))?
            != serde_json::to_vec(&expected).map_err(|e| bad(e.to_string()))?
    {
        return Err(bad("canonical source-bound prefix reconstruction mismatch"));
    }
    Ok(())
}

/// Diagnostic lift only until the sampling and resource contracts close the full PS.
/// No WGSL-valid/full-terrain result is returned on that unresolved path.
pub fn fragment_diagnostic(s: &Sources) -> Result<Value, ShaderError> {
    let p = iridia_dxbc::lift(s.code(Stage::Fragment)?).map_err(|e| bad(e.to_string()))?;
    let validation = p
        .validate()
        .map(|_| "PASS".to_string())
        .unwrap_or_else(|e| format!("{e:?}"));
    Ok(
        json!({"original":terrain::profile(Stage::Fragment),"interface":p.interface,
        "provenance_source_hash":p.provenance.source_hash,"notes":p.notes,
        "validation":validation,
        "full_pixel_resource_contract_admitted":false,"full_pair_ready":false}),
    )
}

