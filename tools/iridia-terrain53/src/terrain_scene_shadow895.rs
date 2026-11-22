//! The original Terrain PS (6bf62d4e) re-projected with the
//! directional shadow array t39 PHYSICAL.
//!
//! Same lift, same Source40 witness and same closed sample walk as the parent
//! projection. The only difference: the 50 original t39 samples (40
//! `sample_c_lz` through comparison sampler s6 and 10 `sample_l` through s3 in
//! the PCSS modes) are NOT replaced by matched-NULL zero results. t39 binds as
//! a depth 2D array at binding 32+39, s3 as an ordinary sampler at 128+3 and s6
//! as a comparison sampler at 128+6 in the parent numbering; the variant binds
//! these three in a SEPARATE group 3 (bindings 0/1/2) so that groups 0..2 stay
//! identical to the parent pipeline layout and the parent's bind groups
//! are reused unchanged. Every other matched-NULL / unobserved
//! site stays projected exactly as in the parent.
//!
//! The shadow filter mode, matrices and texel rows are CB10 bank values, so
//! the program is mode-independent; the original control flow over CB10[31].w
//! and CB10[33].w (modes 0..5) is kept unchanged. With CB10[31].w = CB10[33].w
//! = 0 the program never reads t39 and reproduces the parent's results.
use super::*;

pub const SHADOW_REVISION: &str = "terrain53-shadow895-t39-physical-v1";
/// Matched-NULL slots of the parent minus t39.
const SHADOW_NULL: &[u32] = &[33, 40, 44, 45, 46, 47, 48];
/// Physically required (group, binding) PS bindings of the re-projected variant:
/// the parent's 15 group-1 bindings plus group 3 = (t39 depth array, s3, s6 comparison).
pub const SHADOW_PIXEL_BINDINGS: [(u32, u32); 18] = [
    (1, 0), (1, 1), (1, 2), (1, 3), (1, 6), (1, 10), (1, 36), (1, 37), (1, 40), (1, 66), (1, 67), (1, 68), (1, 69), (1, 128), (1, 129),
    (3, 0), (3, 1), (3, 2),
];
pub const SHADOW_GROUP: u32 = 3;
pub const T39_SAMPLES: usize = 50;

fn kept(src: &OriginalSample) -> bool {
    src.slot == 39 && src.matched_null
}

fn projected_shadow(s: &Sources) -> Result<(iridia::Program, Vec<ProjectionSite>, usize), ShaderError> {
    let source = original_samples(s)?;
    let n39 = source.iter().filter(|r| kept(r)).count();
    if n39 != T39_SAMPLES
        || source.iter().filter(|r| r.slot == 39 && r.operation == "sample_c_lz" && r.sampler == 6).count() != 40
        || source.iter().filter(|r| r.slot == 39 && r.operation == "sample_l" && r.sampler == 3).count() != 10
    {
        return Err(bad("closed original t39 walk: 40 sample_c_lz/s6 + 10 sample_l/s3"));
    }
    let mut p = source_program(s)?;
    let mut global = BTreeMap::new();
    for r in &p.interface.resources {
        let d = r.provenance.d3d.ok_or_else(|| bad("all original resource provenance"))?;
        if d.space != 0 || r.count != 1 {
            return Err(bad("resource array/space outside closed source"));
        }
        let g = p.global(r.id).ok_or_else(|| bad("actual global for resource"))?;
        global.insert(g.index(), (d.class, d.register));
    }
    for (_, f) in p.module.functions.iter() {
        if f.expressions.iter().any(|(_, e)| matches!(e, E::ImageSample { .. } | E::ImageLoad { .. } | E::ImageQuery { .. })) {
            return Err(bad("foreign sampled helper escape"));
        }
    }
    let f = &p.module.entry_points[0].function;
    let mut samples = vec![];
    let image_reg = |f: &ir::Function, h: ir::Handle<E>| -> Result<(iridia::D3dClass, u32), ShaderError> {
        let E::GlobalVariable(g) = f.expressions[h] else {
            return Err(bad("direct source image/sampler handle closure"));
        };
        global.get(&g.index()).copied().ok_or_else(|| bad("original handle provenance"))
    };
    for (h, e) in f.expressions.iter() {
        match e {
            E::ImageSample { image, sampler, depth_ref, level, gather, clamp_to_edge, .. } => {
                if gather.is_some() || *clamp_to_edge {
                    return Err(bad("unknown sample form"));
                }
                let (c, slot) = image_reg(f, *image)?;
                let (sc, sampler) = image_reg(f, *sampler)?;
                if c != iridia::D3dClass::ShaderResource || sc != iridia::D3dClass::Sampler {
                    return Err(bad("source handle kind"));
                }
                let op = match (depth_ref.is_some(), level) {
                    (true, ir::SampleLevel::Zero) => "sample_c_lz",
                    (false, ir::SampleLevel::Exact(_)) => "sample_l",
                    (false, ir::SampleLevel::Gradient { .. }) => "sample_d",
                    (false, ir::SampleLevel::Auto) => "sample",
                    _ => return Err(bad("source sample operation mismatch")),
                };
                samples.push((h, slot, sampler, op));
            }
            E::ImageLoad { image, .. } | E::ImageQuery { image, .. } => {
                let (_, slot) = image_reg(f, *image)?;
                if NULL.contains(&slot) || UNOBS.contains(&slot) {
                    return Err(bad("NULL/unobserved image query/load"));
                }
            }
            _ => {}
        }
    }
    if samples.len() != source.len() {
        return Err(bad("complete75source-to-IR sample ordering"));
    }
    // t39 must be the depth 2D array the comparison reads require.
    let t39_global = p
        .interface
        .resources
        .iter()
        .find(|r| r.provenance.d3d.is_some_and(|d| d.class == iridia::D3dClass::ShaderResource && d.register == 39))
        .and_then(|r| p.global(r.id))
        .ok_or_else(|| bad("t39 global"))?;
    if !matches!(
        p.module.types[p.module.global_variables[t39_global].ty].inner,
        ir::TypeInner::Image { dim: ir::ImageDimension::D2, arrayed: true, class: ir::ImageClass::Depth { multi: false } }
    ) {
        return Err(bad("lifted t39 is not a single-sample depth 2D array"));
    }
    let before = f.clone();
    let mut edits = vec![];
    for ((h, slot, sampler, op), src) in samples.into_iter().zip(&source) {
        if (slot, sampler, op) != (src.slot, src.sampler, src.operation.as_str()) {
            return Err(bad("original sample/slot/sampler exact ordered projection"));
        }
        if kept(src) || !(src.unobserved || src.matched_null) {
            continue;
        }
        if !(src.unobserved || SHADOW_NULL.contains(&slot)) {
            return Err(bad("projected site outside the shadow NULL set"));
        }
        let E::ImageSample { image, .. } = before.expressions[h] else { unreachable!() };
        let E::GlobalVariable(g) = before.expressions[image] else { unreachable!() };
        let depth = matches!(
            p.module.types[p.module.global_variables[g].ty].inner,
            ir::TypeInner::Image { class: ir::ImageClass::Depth { .. }, .. }
        );
        let wanted = if depth {
            ir::TypeInner::Scalar(ir::Scalar::F32)
        } else {
            ir::TypeInner::Vector { size: ir::VectorSize::Quad, scalar: ir::Scalar::F32 }
        };
        let existing = p.module.types.iter().find_map(|(t, v)| (v.inner == wanted).then_some(t));
        let new_result_type_declaration = existing.is_none();
        let ty = existing.unwrap_or_else(|| p.module.types.insert(ir::Type { name: None, inner: wanted }, ir::Span::UNDEFINED));
        edits.push((
            h,
            ty,
            ProjectionSite {
                expression: h.index(),
                source_word: src.word,
                slot,
                kind: if src.unobserved { "UNOBSERVED" } else { "MATCHING_NULL" },
                zero_type: ty.index(),
                new_result_type_declaration,
            },
        ));
    }
    if edits.len() != 71 - T39_SAMPLES {
        return Err(bad("exact21 typed result projection (71 parent sites minus 50 physical t39 samples)"));
    }
    let skip: BTreeSet<_> = edits.iter().map(|(h, _, _)| h.index()).collect();
    let f = &mut p.module.entry_points[0].function;
    for (h, ty, _) in &edits {
        f.expressions[*h] = E::ZeroValue(*ty);
    }
    fix_emits(&mut f.body, &skip);
    let mut a = vec![];
    let mut b = vec![];
    filtered_scope(&before.body, &skip, &mut a);
    filtered_scope(&f.body, &BTreeSet::new(), &mut b);
    if a != b || before.expressions.len() != f.expressions.len() {
        return Err(bad("all control/stores/other emits preserved"));
    }
    for (h, e) in before.expressions.iter() {
        if !skip.contains(&h.index()) && format!("{e:?}") != format!("{:?}", f.expressions[h]) {
            return Err(bad("original numerical/transport expression changed"));
        }
    }
    p.validate().map_err(|e| bad(format!("projected IR validation: {e:?}")))?;
    Ok((p, edits.into_iter().map(|(_, _, r)| r).collect(), n39))
}

fn contract_shadow(p: &iridia::Program) -> Result<rc::CallerContract, ShaderError> {
    use rc::{RegisterClass as C, ResourceContractKind as K, UnreadReason as U};
    let parent = contract_unbound39(p)?;
    let mut rows = vec![];
    for mut r in parent.resources {
        match (r.class, r.register) {
            (C::ShaderResource, 39) => {
                r.resource = K::Texture { dimension: "d2".into(), arrayed: true, multisampled: false, sample: "depth".into() };
                (r.group, r.binding) = (SHADOW_GROUP, 0);
            }
            (C::Sampler, 3) => {
                r.resource = K::Sampler { comparison: false };
                (r.group, r.binding) = (SHADOW_GROUP, 1);
            }
            (C::Sampler, 6) => {
                r.resource = K::Sampler { comparison: true };
                (r.group, r.binding) = (SHADOW_GROUP, 2);
            }
            (C::Sampler, 4 | 7) => {
                if !matches!(r.resource, K::UnreadDeclaration { reason: U::UnusedProjectionDependency, .. }) {
                    return Err(bad("s4/s7 stay unused projection dependencies"));
                }
            }
            _ => {}
        }
        rows.push(r);
    }
    Ok(rc::CallerContract { revision: rc::RESOURCE_REVISION.into(), resources: rows })
}
/// The parent contract over the shadow program (t39 still classified as an
/// unread declaration there; replaced by [`contract_shadow`]).
fn contract_unbound39(p: &iridia::Program) -> Result<rc::CallerContract, ShaderError> {
    contract(p)
}

/// Re-projected pixel payload. Same product boundary as [`PreparedPixel`].
/// Witness development check: Scene40 seal, then the SAME source-only
/// preparation; only `source_scope` carries the witness receipt.
pub fn prepare_shadow_pixel(s: &Sources, w: &SceneWitness) -> Result<PreparedPixel, ShaderError> {
    w.validate()?;
    let mut p = prepare_shadow_pixel_from_sources(s)?;
    p.source_scope = w.receipt();
    Ok(p)
}
/// Shadow receiver pixel preparation from the original sources alone.
pub fn prepare_shadow_pixel_from_sources(s: &Sources) -> Result<PreparedPixel, ShaderError> {
    let (mut p, sites, n39) = projected_shadow(s)?;
    let c = contract_shadow(&p)?;
    let original_notes = p.notes.clone();
    let function_bodies = format!("{:?} {:?}", p.module.functions, p.module.entry_points);
    let manifest = rc::apply_resources(&mut p, &c).map_err(bad)?;
    if function_bodies != format!("{:?} {:?}", p.module.functions, p.module.entry_points) {
        return Err(bad("resource contract changed Terrain function bodies"));
    }
    let physical: Vec<(u32, u32)> = manifest.resources.iter().filter(|r| r.physical_binding_required).map(|r| (r.group, r.binding)).collect::<BTreeSet<_>>().into_iter().collect();
    if physical != SHADOW_PIXEL_BINDINGS {
        return Err(bad(format!("exact18 physically required PS bindings, got {physical:?}")));
    }
    p.validate().map_err(|e| bad(format!("bound pixel validation: {e:?}")))?;
    let unresolved = p.notes.iter().filter(|note| !manifest.resolved_notes.iter().any(|r| &r.original_note == *note)).collect::<Vec<_>>();
    if !unresolved.is_empty() {
        return Err(bad(format!("unresolved pixel lifter notes: {unresolved:?}")));
    }
    let io = iridia::entry_io(&p.interface).map_err(bad)?;
    let mut module = p.module.clone();
    let f = &mut module.entry_points[0].function;
    let mut linked = vec![];
    if f.arguments.len() != io.arguments.len() {
        return Err(bad("exact original PS arguments"));
    }
    for (arg, item) in f.arguments.iter_mut().zip(io.arguments) {
        if let Some(ir::Binding::Location { location, .. }) = &mut arg.binding {
            if item.varyings.len() != 1 {
                return Err(bad("one source register per input"));
            }
            let v = &p.interface.inputs[item.varyings[0]];
            let d = v.provenance.d3d.ok_or_else(|| bad("source varying provenance"))?;
            if d.register == 0 || d.register > 5 || v.value.scalar != iridia::ScalarType::Float || v.value.width != d2r_shader::terrain::WIDTHS[d.register as usize] {
                return Err(bad("exact-width original PS/402 interface"));
            }
            linked.push(json!({"semantic":v.label,"old_location":*location,"original_register_location":d.register,"width":v.value.width,"used":v.used}));
            *location = d.register;
        }
    }
    let info = ir::valid::Validator::new(ir::valid::ValidationFlags::all(), ir::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| bad(e.to_string()))?;
    let wgsl = ir::back::wgsl::write_string(&module, &info, ir::back::wgsl::WriterFlags::empty()).map_err(|e| bad(e.to_string()))?;
    let stock = naga::front::wgsl::parse_str(&wgsl).map_err(|e| bad(e.emit_to_string(&wgsl)))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&stock)
        .map_err(|e| bad(e.to_string()))?;
    let preparation_key = sha256(
        &serde_json::to_vec(&json!({"revision":REVISION,"shadow_revision":SHADOW_REVISION,
        "original_sources":s.canonical.identities(),"locked_packages_sha256":sha256(include_bytes!("../Cargo.lock")),
        "sampling51":sample_lod::SAMPLE_LOD_REVISION,"resources52":rc::RESOURCE_REVISION,
        "null_capabilities":source_null_capabilities(),"physical_t39":true,"actual_bank_predicate":"PSb2[20].x==2/bit16off",
        "static_bindings":manifest,"wgsl_sha256":sha256(wgsl.as_bytes()),
        "source_bank45_sha256":SOURCE_BANK45_SHA256,
        "dynamic_frame_banks_lights_images_not_shader_key":true}))
        .map_err(|e| bad(e.to_string()))?,
    );
    Ok(PreparedPixel {
        revision: SHADOW_REVISION,
        preparation_key,
        wgsl_sha256: sha256(wgsl.as_bytes()),
        wgsl,
        bindings: serde_json::to_value(manifest).map_err(|e| bad(e.to_string()))?,
        source_scope: source_only_scope(),
        projection: json!({"parent_revision":REVISION,"sites":sites,"source_samples":75,"physical_t39_samples":n39,
            "matched_null":63 - n39,"unobserved":8,"control_stores_other_expressions_exact":true,
            "original_iridia_notes":original_notes,"remaining_iridia_notes":p.notes,"unclassified_notes":[]}),
        linking: json!({"location_only":linked,"padding":false,"math_body_rewritten":false,"original_outputs":"o0/o1float4; no velocityo2"}),
        source_math_control_preserved: true,
        physical_null_substitutes: false,
        current_world_or_gpu_authority: false,
    })
}

