//! Generic preparation mechanics; the product retains its existing source
//! catalogue as the sole profile authority. A prepared artifact is not a live
//! resource/current-frame permit.
use crate::caller_projection::{CallerCapability, apply_caller_projection};
use crate::prepare::{bound_program, source_declarations};
use crate::resource_contract::{CallerContract, apply_resources};
use crate::stage_link::{DrawContract, LinkManifest, apply_draw_contract};
use crate::{
    CacheIdentity, Error, PreparedShader, Result, SourceIdentity, SourceShader, Stage,
    TargetContract,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
pub const GRAPHICS_SCHEMA: &str = "d2r-iridia50-graphics-v1";

/// Derive only the physical-binding omissions justified by a sealed source
/// caller projection. The returned contract is serializable evidence, not the
/// capability: preparation still requires `cap` and repeats its source proof.
pub fn projected_caller_contract(
    source: &SourceShader,
    caller: &CallerContract,
    cap: &CallerCapability,
) -> Result<CallerContract> {
    let (mut p, _) = if cap.evidence().profile == crate::caller_projection::water_policy649::PROFILE {
        crate::caller_projection::water_policy649::check_cap(cap)?;
        crate::caller_projection::water_policy649::check_source(source.identity())?;
        crate::caller_projection::water_policy649::check_contract(source.identity().stage, caller, false)?;
        crate::prepare::bound_controlled_program104(source, caller)?
    } else if cap.evidence().profile == crate::caller_projection::foliage_policy883::PROFILE {
        crate::caller_projection::foliage_policy883::source(source.identity())?;
        if source.identity().stage == Stage::Vertex {
            crate::caller_projection::foliage_policy883::vertex_contract(&source.identity().dxbc_sha256, caller)?;
        } else {
            crate::caller_projection::foliage_policy883::contract(source.identity().stage, caller, false)?;
        }
        crate::prepare::bound_controlled_program104(source, caller)?
    } else { bound_program(source, caller)? };
    projected_program_contract(&mut p, caller, cap).map(|(contract, _)| contract)
}
fn projected_program_contract(
    p: &mut iridia::Program,
    caller: &CallerContract,
    cap: &CallerCapability,
) -> Result<(
    CallerContract,
    crate::caller_projection::CallerProjectionReport,
)> {
    projected_program_contract_impl438(p, caller, cap, None)
}
fn projected_program_contract_impl438(
    p: &mut iridia::Program,
    caller: &CallerContract,
    cap: &CallerCapability,
    background: Option<&crate::background_guard438::BackgroundGuardCapability438>,
) -> Result<(
    CallerContract,
    crate::caller_projection::CallerProjectionReport,
)> {
    use crate::resource_contract::{RegisterClass, ResourceContractKind as K, UnreadReason};
    use iridia_naga::Expression as E;
    use std::collections::BTreeSet;
    let mut projected_registers = cap
        .evidence()
        .fixtures
        .iter()
        .map(|f| f.binding - 32)
        .collect::<BTreeSet<_>>();
    if background.is_some() {
        projected_registers.extend([30, 31, 32, 33, 40]);
    }
    let globals = p
        .interface
        .resources
        .iter()
        .filter_map(|r| Some((p.global(r.id)?, r.provenance.d3d?, r.id)))
        .collect::<Vec<_>>();
    let mut sampler_dependencies = BTreeSet::new();
    for f in p
        .module
        .functions
        .iter()
        .map(|(_, f)| f)
        .chain(p.module.entry_points.iter().map(|e| &e.function))
    {
        for (_, e) in f.expressions.iter() {
            if let E::ImageSample { image, sampler, .. } = e {
                if let (E::GlobalVariable(image), E::GlobalVariable(sampler)) =
                    (&f.expressions[*image], &f.expressions[*sampler])
                {
                    if globals.iter().any(|(g, d, _)| {
                        g == image
                            && d.class == iridia::D3dClass::ShaderResource
                            && projected_registers.contains(&d.register)
                    }) {
                        if let Some((_, d, id)) = globals
                            .iter()
                            .find(|(g, d, _)| g == sampler && d.class == iridia::D3dClass::Sampler)
                        {
                            sampler_dependencies.insert((*id, d.register));
                        }
                    }
                }
            }
        }
    }
    let report = if let Some(background) = background {
        crate::background_guard438::apply(p, background)?
    } else {
        apply_caller_projection(p, cap).map_err(Error::Unsupported)?
    };
    if !report.complete_resource_projection || report.grants_current_host_authority {
        return Err(Error::Unsupported("incomplete caller projection".into()));
    }
    let unread = p
        .unread_resources()
        .map_err(|e| Error::Validation(e.to_string()))?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut out = caller.clone();
    for c in &mut out.resources {
        let r = p
            .interface
            .resources
            .iter()
            .find(|r| {
                r.provenance.d3d.is_some_and(|d| {
                    d.register == c.register
                        && d.space == c.space
                        && match c.class {
                            RegisterClass::ConstantBuffer => {
                                d.class == iridia::D3dClass::ConstantBuffer
                            }
                            RegisterClass::ShaderResource => {
                                d.class == iridia::D3dClass::ShaderResource
                            }
                            RegisterClass::Sampler => d.class == iridia::D3dClass::Sampler,
                            RegisterClass::UnorderedAccess => {
                                d.class == iridia::D3dClass::UnorderedAccess
                            }
                        }
                })
            })
            .ok_or_else(|| Error::Identity("caller source declaration absent".into()))?;
        let reason = if let Some(r) = report
            .unread_resources
            .iter()
            .find(|u| u.resource_id == r.id.0)
        {
            Some(match r.reason.as_str() {
                "MatchedNull" => UnreadReason::MatchedNull,
                "UnobservedSourceBranch" => UnreadReason::UnobservedSourceBranch,
                _ => return Err(Error::Unsupported("unknown caller unread reason".into())),
            })
        } else if c.class == RegisterClass::Sampler
            && sampler_dependencies.contains(&(r.id, c.register))
            && unread.contains(&r.id)
        {
            Some(UnreadReason::UnusedProjectionDependency)
        } else {
            None
        };
        if let Some(reason) = reason {
            c.resource = K::UnreadDeclaration {
                original_resource_sha256: crate::resource_contract::source_declaration_sha256(r)
                    .map_err(Error::Invalid)?,
                reason,
            };
        } else if matches!(c.resource, K::UnreadDeclaration { .. }) {
            return Err(Error::Unsupported(
                "unread declaration is not justified by this source caller projection".into(),
            ));
        }
    }
    Ok((out, report))
}

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
    /// Internal source/caller/WGSL consistency, not catalogue or GPU authority.
    pub fn validate_artifact(&self) -> Result<()> {
        if self.schema != GRAPHICS_SCHEMA
            || self.profile.is_empty()
            || self.profile.len() > 512
            || !self.profile.is_ascii()
            || !self.requires_host_profile_admission
            || self.vertex.stage() != Stage::Vertex
            || self.fragment.stage() != Stage::Fragment
        {
            return Err(Error::Identity(
                "graphics schema/profile/stage/authority qualification".into(),
            ));
        }
        self.vertex.validate_artifact()?;
        self.fragment.validate_artifact()?;
        if self.profile==crate::caller_projection::background_policy677::PROFILE
            && self.vertex.caller_contract()["background677"]!=self.fragment.caller_contract()["background677"] {
            return Err(Error::Identity("Background677 paired draw/bank/light/camera provenance differs".into()));
        }
        if self.profile == crate::caller_projection::standard_policy668::PROFILE
            && self.vertex.caller_contract()["standard668"] != self.fragment.caller_contract()["standard668"] {
            return Err(Error::Identity("Standard668 pair source/bank/resource receipts differ".into()));
        }
        if self.profile == crate::caller_projection::foliage_policy883::PROFILE
            && self.vertex.caller_contract()[crate::caller_projection::foliage_policy883::RECEIPT_KEY]
                != self.fragment.caller_contract()[crate::caller_projection::foliage_policy883::RECEIPT_KEY] {
            return Err(Error::Identity("Foliage23 pair source/host-grid receipts differ".into()));
        }
        if self.profile.starts_with("authored-idle830:") {
            let v:PreparedShader=serde_json::from_value(self.vertex.caller_contract()["authored830"]["base_original_artifact"].clone())?;
            let f:PreparedShader=serde_json::from_value(self.fragment.caller_contract()["authored830"]["base_original_artifact"].clone())?;
            if self.vertex.caller_contract()["authored830"]["domain"]!=self.fragment.caller_contract()["authored830"]["domain"] {return Err(Error::Identity("830 paired light domain mismatch".into()));}
            let original=PreparedGraphics{schema:self.schema.clone(),profile:v.caller_contract()["profile"].as_str().ok_or_else(||Error::Identity("830 source profile".into()))?.into(),requires_host_profile_admission:true,vertex:v,fragment:f,linkage:self.linkage.clone()};
            original.validate_artifact()?;
        }
        let link = serde_json::to_value(&self.linkage)?;
        if self.vertex.linked_interface() != &link
            || self.fragment.linked_interface() != &link
            || self.vertex.target_capabilities() != self.fragment.target_capabilities()
            || self.vertex.caller_contract()["profile"].as_str() != Some(self.profile.as_str())
            || self.fragment.caller_contract()["profile"].as_str() != Some(self.profile.as_str())
        {
            return Err(Error::Identity("graphics pair context differs".into()));
        }
        crate::validation::validate_graphics_linkage(&self.vertex, &self.fragment, &self.linkage)
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
    prepare_graphics_impl104(
        profile,
        vs,
        ps,
        vertex,
        fragment,
        draw,
        target,
        fragment_projection,
        false,
        None,
        None,
        None,
        None,
    )
}
/// Explicit receipt-checked controlled preparation, still requiring independent host admission.
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
    prepare_graphics_impl104(
        profile,
        vs,
        ps,
        vertex,
        fragment,
        draw,
        target,
        fragment_projection,
        true,
        None,
        None,
        None,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
fn prepare_graphics_impl104(
    profile: &str,
    vs: &SourceShader,
    ps: &SourceShader,
    vertex: &CallerContract,
    fragment: &CallerContract,
    draw: &DrawContract,
    target: &TargetContract,
    fragment_projection: Option<&CallerCapability>,
    controlled: bool,
    background: Option<(
        &std::sync::Arc<crate::background_guard438::BackgroundOwnedInputs438>,
        &crate::background_guard438::BackgroundGuardCapability438,
    )>,
    standard: Option<(
        &std::sync::Arc<crate::caller_projection::standard_policy668::StandardSource668>,
        &crate::caller_projection::standard_policy668::StandardCapability668,
    )>,
    active_background: Option<(
        &crate::caller_projection::background_policy677::BackgroundDraw677,
        &crate::caller_projection::background_policy677::BackgroundCapability677,
    )>,
    foliage: Option<&crate::caller_projection::foliage_policy883::Foliage23Capability>,
) -> Result<PreparedGraphics> {
    if let Some((owner, cap)) = background {
        cap.recheck(owner, vs, ps)?;
    }
    if profile.is_empty()
        || profile.len() > 512
        || !profile.is_ascii()
        || vs.identity().stage != Stage::Vertex
        || ps.identity().stage != Stage::Fragment
    {
        return Err(Error::Unsupported(
            "graphics profile/stage selection".into(),
        ));
    }
    let background677 = if profile == crate::caller_projection::background_policy677::PROFILE {
        let (draw677,cap)=active_background.ok_or_else(||Error::Identity("Background677 requires source-sealed ACTIVE19 draw".into()))?;
        if !controlled || background.is_some() || standard.is_some() || !fragment_projection.is_some_and(|p|std::ptr::eq(p,cap.projection())) {
            return Err(Error::Identity("Background677 exact controlled source capability".into()));
        }
        cap.recheck(draw677,vs,ps)?;
        crate::caller_projection::background_policy677::contract(Stage::Vertex,vertex,false)?;
        crate::caller_projection::background_policy677::contract(Stage::Fragment,fragment,true)?;
        Some(cap.receipt()?)
    } else {
        if active_background.is_some(){return Err(Error::Identity("Background677 owner on another profile".into()));}
        None
    };
    let standard668 = if profile == crate::caller_projection::standard_policy668::PROFILE {
        let (owner, cap) = standard.ok_or_else(|| Error::Identity("Standard668 requires sealed typed source constructor".into()))?;
        if !controlled || background.is_some() || !fragment_projection.is_some_and(|p|std::ptr::eq(p,cap.projection())) {
            return Err(Error::Identity("Standard668 exact controlled projection capability required".into()));
        }
        cap.recheck(owner, vs, ps)?;
        crate::caller_projection::standard_policy668::contract(Stage::Vertex, vertex, false)?;
        crate::caller_projection::standard_policy668::contract(Stage::Fragment, fragment, true)?;
        Some(cap.receipt()?)
    } else {
        if standard.is_some(){return Err(Error::Identity("Standard668 source on another profile".into()));}
        None
    };
    // The separately sealed map Standard authority (standard_map1129).
    let map_standard1129 = if profile == crate::caller_projection::standard_map1129::PROFILE {
        use crate::caller_projection::standard_map1129 as map;
        let cap = fragment_projection.ok_or_else(|| Error::Identity("MapStandard1129 sealed map capability required".into()))?;
        if !controlled || background.is_some() || standard.is_some() || active_background.is_some() || foliage.is_some() {
            return Err(Error::Identity("MapStandard1129 exact controlled projection capability required".into()));
        }
        map::evidence(cap.evidence())?;
        map::contract(Stage::Vertex, vertex, false)?;
        map::contract(Stage::Fragment, fragment, true)?;
        Some(map::impl_receipt())
    } else { None };
    let water649 = if profile == crate::caller_projection::water_policy649::PROFILE {
        if !controlled || background.is_some() { return Err(Error::Identity("Water649 requires controlled original preparation".into())); }
        let cap = fragment_projection.ok_or_else(|| Error::Identity("Water649 sealed source projection required".into()))?;
        let evidence = crate::caller_projection::water_policy649::check_cap(cap)?.clone();
        crate::caller_projection::water_policy649::check_source(vs.identity())?;
        crate::caller_projection::water_policy649::check_source(ps.identity())?;
        crate::caller_projection::water_policy649::check_contract(Stage::Vertex, vertex, false)?;
        crate::caller_projection::water_policy649::check_contract(Stage::Fragment, fragment, true)?;
        Some(evidence)
    } else { None };
    let foliage23 = if profile == crate::caller_projection::foliage_policy883::PROFILE {
        use crate::caller_projection::foliage_policy883 as foliage883;
        let cap = foliage.ok_or_else(|| Error::Identity("Foliage23 requires the sealed source-pair/host-grid capability".into()))?;
        if !controlled || background.is_some() || standard.is_some() || active_background.is_some()
            || !fragment_projection.is_some_and(|p| std::ptr::eq(p, cap.projection())) {
            return Err(Error::Identity("Foliage23 exact controlled projection capability required".into()));
        }
        cap.recheck(vs, ps)?;
        foliage883::vertex_contract(&vs.identity().dxbc_sha256, vertex)?;
        foliage883::contract_with(Stage::Fragment, fragment, true, cap.unit_lights().map(|u| u.t8_elements))?;
        Some(cap.receipt())
    } else {
        if foliage.is_some() { return Err(Error::Identity("Foliage23 capability on another profile".into())); }
        None
    };
    target.validate()?;
    target.resources(vertex)?;
    target.resources(fragment)?;
    use crate::resource_contract::ResourceContractKind as K;
    if vertex
        .resources
        .iter()
        .any(|r| matches!(r.resource, K::UnreadDeclaration { .. }))
        || (fragment_projection.is_none()
            && fragment
                .resources
                .iter()
                .any(|r| matches!(r.resource, K::UnreadDeclaration { .. })))
    {
        return Err(Error::Unsupported(
            "unread declarations require the supported sealed fragment caller projection".into(),
        ));
    }
    let prepare = if controlled {
        crate::prepare::bound_controlled_program104
    } else {
        bound_program
    };
    let (mut v, vsampling) = prepare(vs, vertex)?;
    let (mut p, psampling) = prepare(ps, fragment)?;
    let vd = source_declarations(&v)?;
    let pd = source_declarations(&p)?;
    let vi = serde_json::to_value(&v.interface)?;
    let pi = serde_json::to_value(&p.interface)?;
    let original_vnotes = v.notes.clone();
    let original_pnotes = p.notes.clone();
    let projection = if let Some(c) = fragment_projection {
        if c.evidence().profile != profile {
            return Err(Error::Identity(
                "caller projection belongs to another profile".into(),
            ));
        }
        let (derived, r) = projected_program_contract_impl438(
            &mut p,
            fragment,
            c,
            background.map(|(_, cap)| cap),
        )?;
        if &derived != fragment {
            return Err(Error::Identity(
                "explicit unread contract differs from recomputed source projection".into(),
            ));
        }
        if !r.complete_resource_projection || r.grants_current_host_authority {
            return Err(Error::Unsupported(
                "incomplete caller projection/authority qualification".into(),
            ));
        }
        Some(r)
    } else {
        None
    };
    let typed_null = if let (Some(cap), Some(report)) = (fragment_projection, projection.as_ref()) {
        let report =
            crate::typed_null_note::resolve_projected_typed_note(&mut p, fragment, cap, report)
                .map_err(Error::Unsupported)?;
        (!report.resolved_notes.is_empty()).then_some(report)
    } else {
        None
    };
    let vb = apply_resources(&mut v, vertex).map_err(Error::Unsupported)?;
    let pb = apply_resources(&mut p, fragment).map_err(Error::Unsupported)?;
    if !v.notes.is_empty() || !p.notes.is_empty() {
        return Err(Error::Unsupported(format!(
            "unresolved original notes after contracts: {:?}/{:?}",
            v.notes, p.notes
        )));
    }
    let dm = apply_draw_contract(&mut v, draw).map_err(Error::Unsupported)?;
    if dm.as_ref().is_some_and(|d| {
        d.group >= target.max_bind_groups
            || d.binding >= target.max_bindings_per_bind_group
            || u64::from(d.uniform_bytes) > target.max_uniform_buffer_binding_size
    }) {
        return Err(Error::Unsupported(
            "draw transport exceeds target limits".into(),
        ));
    }
    let linked = crate::stage_link::link_payload_pair104(&v, &p, &draw.topology, controlled)
        .map_err(Error::Unsupported)?;
    let target_json = serde_json::to_value(target)?;
    let linkage = serde_json::to_value(&linked.manifest)?;
    let mut vc = json!({"profile":profile,"resources":vertex,"draw":draw,"catalogue_admission_required":true});
    let mut pc = json!({"profile":profile,"resources":fragment,"draw":null,"projection":projection.as_ref().map(|r|&r.evidence),"catalogue_admission_required":true});
    if let Some(evidence) = background677 {
        vc["background677"]=evidence.clone();pc["background677"]=evidence;
    }
    if let Some(evidence) = standard668 {
        vc["standard668"] = evidence.clone();
        pc["standard668"] = evidence;
    }
    if let Some(receipt) = map_standard1129.clone() {
        vc[crate::caller_projection::standard_map1129::RECEIPT_KEY] = receipt.clone();
        pc[crate::caller_projection::standard_map1129::RECEIPT_KEY] = receipt;
    }
    if let Some(receipt) = foliage23 {
        vc[crate::caller_projection::foliage_policy883::RECEIPT_KEY] = receipt.clone();
        pc[crate::caller_projection::foliage_policy883::RECEIPT_KEY] = receipt;
    }
    if let Some(evidence) = water649 {
        vc["water649"] = evidence.clone();
        pc["water649"] = evidence;
    }
    if let Some((_, cap)) = background {
        vc["background438"] = cap.evidence();
        pc["background438"] = cap.evidence();
    }
    let mut passes = BTreeMap::from([
        (crate::resource_contract::RESOURCE_REVISION.to_string(), 2),
        (crate::stage_link::LINK_REVISION.to_string(), 1),
        (crate::stage_link::DRAW_REVISION.to_string(), 1),
        (crate::sample_lod::SAMPLE_LOD_REVISION.to_string(), 2),
    ]);
    if active_background.is_some(){passes.insert(crate::caller_projection::background_policy677::RULE.into(),1);}
    if foliage.is_some(){passes.insert(crate::caller_projection::foliage_policy883::RULE.into(),1);}
    if map_standard1129.is_some(){passes.insert(crate::caller_projection::standard_map1129::RULE.into(),1);}
    if standard.is_some() {
        passes.insert(crate::caller_projection::standard_policy668::RULE.into(), 1);
    }
    if projection.is_some() {
        passes.insert(crate::caller_projection::CALLER_REVISION.to_string(), 3);
    }
    if background.is_some() {
        passes.insert(crate::background_guard438::BACKGROUND_REVISION438.into(), 1);
    }
    if typed_null.is_some() {
        passes.insert(
            crate::typed_null_note::TYPED_NULL_NOTE_REVISION.to_string(),
            1,
        );
    }
    let vm = json!({"stage":"vertex","entry_point":v.entry,"original_interface":vi,"prepared_interface":v.interface,"source_declarations":vd,"bindings":vb,"draw":dm,"sampling":vsampling,"original_notes":original_vnotes,"outstanding_notes":v.notes,"stock_naga_validated":true,"gpu_verified":false});
    let mut pm = json!({"stage":"fragment","entry_point":p.entry,"original_interface":pi,"prepared_interface":p.interface,"source_declarations":pd,"bindings":pb,"draw":null,"sampling":psampling,"caller_projection":projection,"typed_null_note":typed_null,"original_notes":original_pnotes,"outstanding_notes":p.notes,"stock_naga_validated":true,"gpu_verified":false});
    // A fragment UAV buffer is admitted only as a source read-only binding; its access
    // transport is reported (and revalidated) exactly as for compute stages. Manifests of
    // fragment stages without storage buffers keep their previous shape.
    if fragment.resources.iter().any(|r| matches!(r.resource, K::StorageBuffer { .. })) {
        pm["storage_buffer_transport"] =
            crate::validation::storage_buffer_transport_module(&linked.fragment.module()?, fragment)?;
    }
    let vid = CacheIdentity::current(
        vs.identity().clone(),
        &vc,
        &linkage,
        &target_json,
        &vm,
        passes.clone(),
    )?;
    let pid = CacheIdentity::current(
        ps.identity().clone(),
        &pc,
        &linkage,
        &target_json,
        &pm,
        passes,
    )?;
    let out = PreparedGraphics {
        schema: GRAPHICS_SCHEMA.into(),
        profile: profile.into(),
        requires_host_profile_admission: true,
        vertex: PreparedShader::checked_payload(
            vid,
            vc,
            linkage.clone(),
            target_json.clone(),
            vm,
            linked.vertex,
        )?,
        fragment: PreparedShader::checked_payload(
            pid,
            pc,
            linkage,
            target_json,
            pm,
            linked.fragment,
        )?,
        linkage: linked.manifest,
    };
    out.validate_artifact()?;
    Ok(out)
}

/// Only the sealed Standard profile admits this
/// profile. It re-derives source projection before the unchanged resource pass.
#[allow(clippy::too_many_arguments)]
pub fn prepare_standard_graphics668(
    owner:&std::sync::Arc<crate::caller_projection::standard_policy668::StandardSource668>,
    draw_index:usize, vs:&SourceShader, ps:&SourceShader,
    vertex:&CallerContract, physical_fragment:&CallerContract,
    draw:&DrawContract, target:&TargetContract,
) -> Result<PreparedGraphics> {
    use crate::caller_projection::standard_policy668 as standard;
    standard::contract(Stage::Vertex,vertex,false)?;
    standard::contract(Stage::Fragment,physical_fragment,false)?;
    let cap=standard::StandardCapability668::seal(owner,draw_index,vs,ps)?;
    let(mut program,_)=crate::prepare::bound_controlled_program104(ps,physical_fragment)?;
    let(fragment,_)=projected_program_contract(&mut program,physical_fragment,cap.projection())?;
    prepare_graphics_impl104(standard::PROFILE,vs,ps,vertex,&fragment,draw,target,
        Some(cap.projection()),true,None,Some((owner,&cap)),None,None)
}

/// The original Standard pair for the streamed map, a separately named authority
/// (`caller_projection::standard_map1129`): the unchanged Standard630 physical caller and
/// source-null projection, sealed from the source pair and the expected NULL image set.
/// The authored-scene profile (`prepare_standard_graphics668`) is unchanged.
pub fn prepare_standard_map_graphics1129(
    vs: &SourceShader, ps: &SourceShader,
    vertex: &CallerContract, physical_fragment: &CallerContract,
    draw: &DrawContract, target: &TargetContract,
    cap: &crate::caller_projection::standard_map1129::MapStandardCapability1129,
) -> Result<PreparedGraphics> {
    use crate::caller_projection::standard_map1129 as map;
    map::contract(Stage::Vertex, vertex, false)?;
    map::contract(Stage::Fragment, physical_fragment, false)?;
    cap.recheck(vs, ps)?;
    let (mut program, _) = crate::prepare::bound_controlled_program104(ps, physical_fragment)?;
    let (fragment, _) = projected_program_contract(&mut program, physical_fragment, cap.projection())?;
    prepare_graphics_impl104(map::PROFILE, vs, ps, vertex, &fragment, draw, target,
        Some(cap.projection()), true, None, None, None, None)
}

/// Three-background profile only.
pub fn prepare_background_graphics677(
    draw677:&crate::caller_projection::background_policy677::BackgroundDraw677,
    vs:&SourceShader,ps:&SourceShader,draw:&DrawContract,target:&TargetContract,
) -> Result<PreparedGraphics> {
    use crate::caller_projection::background_policy677 as current;
    let [vertex,physical_fragment]=draw677.physical_callers()?;
    let cap=current::BackgroundCapability677::seal(draw677,vs,ps)?;
    let(mut program,_)=crate::prepare::bound_controlled_program104(ps,&physical_fragment)?;
    let(fragment,_)=projected_program_contract(&mut program,&physical_fragment,cap.projection())?;
    prepare_graphics_impl104(current::PROFILE,vs,ps,&vertex,&fragment,draw,target,
        Some(cap.projection()),true,None,None,Some((draw677,&cap)),None)
}

/// Original Foliage23 pair with the expected NULL image set. Derives the projected fragment contract from
/// the physical caller and the sealed capability, then repeats it inside preparation.
#[allow(clippy::too_many_arguments)]
pub fn prepare_foliage_graphics883(
    vs: &SourceShader, ps: &SourceShader,
    vertex: &CallerContract, physical_fragment: &CallerContract,
    draw: &DrawContract, target: &TargetContract,
    cap: &crate::caller_projection::foliage_policy883::Foliage23Capability,
) -> Result<PreparedGraphics> {
    use crate::caller_projection::foliage_policy883 as foliage883;
    foliage883::vertex_contract(&vs.identity().dxbc_sha256, vertex)?;
    cap.check_caller(physical_fragment)?;
    cap.recheck(vs, ps)?;
    let (mut program, _) = crate::prepare::bound_controlled_program104(ps, physical_fragment)?;
    let (fragment, _) = projected_program_contract(&mut program, physical_fragment, cap.projection())?;
    prepare_graphics_impl104(foliage883::PROFILE, vs, ps, vertex, &fragment, draw, target,
        Some(cap.projection()), true, None, None, None, Some(cap))
}
