//! Terrain-only closed source/caller projection. No physical NULL substitutes.
use super::{Sources, bad};
use super::{resource_contract as rc, sample_lod};
use d2r_shader::{ShaderError, sha256};
use ir::{Expression as E, Statement as S};
use iridia_naga as ir;
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub const REVISION: &str = "terrain53-actual-scene40-bank16off-71-result-projection-v1";
const NULL: &[u32] = &[33, 39, 40, 44, 45, 46, 47, 48];
const UNOBS: &[u32] = &[30, 31, 32];
/// SHA-256 of the Bank45 source input; part of the preparation key.
const SOURCE_BANK45_SHA256: &str = "21007827dddb55b6876967ba355139956ce450442caafe9032451e0f3a0b789f";

fn source_null_capabilities() -> Value {
    json!([30u32,31,32,33,39,40,44,45,46,47,48].map(|slot|json!({
        "slot":slot,"resource_is_null":true,
        "dimension":match slot{33=>"TextureCube",39|40=>"Texture2DArray",45..=48=>"Texture3D",_=>"Texture2D"},
        "format":"R32G32B32A32_FLOAT","array_layers":if [39,40].contains(&slot){1}else{0},
        "mip_levels":1,"first_array_slice":0,"most_detailed_mip":0,
        "shader_component_mapping":5768,"plane_slice":0,"resource_min_lod_clamp_bits":0,
        "result_rule":if UNOBS.contains(&slot){"UNOBSERVED under actual flag bit16 off; no mismatched NULL zero semantics"}else{"Separate source-compatible operation-specific NULL-result proof"}
    })))
}
#[derive(Clone, Debug, Serialize)]
struct OriginalSample {
    word: usize,
    slot: u32,
    sampler: u32,
    operation: String,
    unobserved: bool,
    matched_null: bool,
}
fn immediate_index(o: &dxbc_reader::Operand) -> Result<u32, ShaderError> {
    if o.indices.len() != 1 || o.indices[0].relative.is_some() {
        return Err(bad("closed immediate source resource index"));
    }
    Ok(o.indices[0].base)
}
fn written_channels_only_x(
    destination: dxbc_reader::Selection,
    resource: dxbc_reader::Selection,
) -> bool {
    use dxbc_reader::Selection;
    let Selection::Mask(mask) = destination else {
        return false;
    };
    if mask == 0 || mask & !15 != 0 {
        return false;
    }
    (0..4)
        .filter(|lane| mask & (1 << lane) != 0)
        .all(|lane| match resource {
            Selection::Scalar(c) => c == 0,
            Selection::Swizzle(channels) => channels[lane] == 0,
            _ => false,
        })
}
fn original_samples(s: &Sources) -> Result<Vec<OriginalSample>, ShaderError> {
    use dxbc_reader::{Register as R, Selection};
    let code = dxbc_reader::code(s.code(d2r_shader::terrain::OriginalTerrainStage::Fragment)?)
        .map_err(|e| bad(e.to_string()))?;
    let decoded = dxbc_reader::decode(&code).map_err(|e| bad(e.to_string()))?;
    let mut stack = vec![];
    let mut rows = vec![];
    let mut gate = false;
    for ins in &decoded.instructions {
        if ins.word == 536 {
            if ins.name() != "and"
                || ins.operands.len() != 3
                || ins.operands[0].register != R::Temp
                || immediate_index(&ins.operands[0])? != 3
                || ins.operands[0].selection != Selection::Mask(6)
                || ins.operands[1].register != R::Constant
                || ins.operands[1].indices.len() != 2
                || ins.operands[1].indices.iter().any(|i| i.relative.is_some())
                || ins.operands[1]
                    .indices
                    .iter()
                    .map(|i| i.base)
                    .collect::<Vec<_>>()
                    != [2, 20]
                || !matches!(
                    ins.operands[1].selection,
                    Selection::Scalar(0) | Selection::Swizzle([0, 0, 0, 0])
                )
                || ins.operands[2].register != R::Immediate
                || ins.operands[2].immediate != [0, 16, 2, 0]
            {
                return Err(bad("complete original AND predicate operands"));
            }
            gate = true;
        }
        match ins.name() {
            "if" => {
                if ins.word == 547
                    && (!gate
                        || !ins.nonzero
                        || ins.operands.len() != 1
                        || ins.operands[0].register != R::Temp
                        || immediate_index(&ins.operands[0])? != 3
                        || !matches!(
                            ins.operands[0].selection,
                            Selection::Scalar(1) | Selection::Swizzle([1, 1, 1, 1])
                        ))
                {
                    return Err(bad("original if_nz flag predicate"));
                }
                stack.push((ins.word, true));
            }
            "else" => {
                stack.last_mut().ok_or_else(|| bad("source else scope"))?.1 = false;
            }
            "endif" => {
                stack.pop().ok_or_else(|| bad("source endif scope"))?;
            }
            _ => {}
        }
        let resource = ins.operands.iter().find(|o| o.register == R::Texture);
        let Some(resource) = resource else {
            continue;
        };
        let slot = immediate_index(resource)?;
        if ins.name().starts_with("dcl_") {
            continue;
        }
        if !["sample", "sample_l", "sample_c_lz", "sample_d"].contains(&ins.name()) {
            if NULL.contains(&slot) || UNOBS.contains(&slot) {
                return Err(bad("NULL/UNOBS source query/load/escape"));
            }
            continue;
        }
        let sampler = ins
            .operands
            .iter()
            .find(|o| o.register == R::Sampler)
            .ok_or_else(|| bad("source sampler"))?;
        let unobserved = UNOBS.contains(&slot);
        if unobserved && (ins.name() != "sample_d" || !stack.contains(&(547, true))) {
            return Err(bad("all8 source arrays require exact false gate"));
        }
        if slot == 39
            && ins.name() == "sample_l"
            && !written_channels_only_x(ins.operands[0].selection, resource.selection)
        {
            return Err(bad("ordinary depth-view NULL source observes only x"));
        }
        rows.push(OriginalSample {
            word: ins.word,
            slot,
            sampler: immediate_index(sampler)?,
            operation: ins.name().into(),
            unobserved,
            matched_null: NULL.contains(&slot),
        });
    }
    if !stack.is_empty()
        || !gate
        || rows.len() != 75
        || rows.iter().filter(|r| r.unobserved).count() != 8
        || rows.iter().filter(|r| r.matched_null).count() != 63
    {
        return Err(bad("complete original75sample closure"));
    }
    Ok(rows)
}
fn source_program(s: &Sources) -> Result<iridia::Program, ShaderError> {
    let bytes = s.code(d2r_shader::terrain::OriginalTerrainStage::Fragment)?;
    let initial = iridia_dxbc::lift(bytes).map_err(|e| bad(e.to_string()))?;
    let mut bound = iridia::BoundSizes::new();
    let mut regs = vec![];
    for r in &initial.interface.resources {
        if let iridia::ResourceKind::ConstantBuffer { .. } = &r.kind {
            let b = r
                .provenance
                .d3d
                .ok_or_else(|| bad("source CB provenance"))?;
            regs.push(b.register);
            bound.set(r.id, 1536);
        }
    }
    regs.sort();
    if regs != [0, 1, 2, 3, 6, 10] {
        return Err(bad("six original actual fullCB views"));
    }
    let mut p = iridia_dxbc::lift_with(bytes, &bound).map_err(|e| bad(e.to_string()))?;
    sample_lod::repair_exact_lod(&mut p).map_err(|e| bad(e.to_string()))?;
    p.validate()
        .map_err(|e| bad(format!("source pixel IR: {e:?}")))?;
    if p.interface.resources.len() != 30 || p.module.entry_points.len() != 1 {
        return Err(bad("complete original30resource/onePSentry contract"));
    }
    Ok(p)
}

#[derive(Debug, Serialize)]
struct ProjectionSite {
    expression: usize,
    source_word: usize,
    slot: u32,
    kind: &'static str,
    zero_type: usize,
    new_result_type_declaration: bool,
}
fn filtered_scope(block: &ir::Block, skip: &BTreeSet<usize>, out: &mut Vec<String>) {
    for s in block {
        match s {
            S::Emit(r) => {
                for h in r.clone() {
                    if !skip.contains(&h.index()) {
                        out.push(format!("emit{}", h.index()));
                    }
                }
            }
            S::Block(b) => {
                out.push("block".into());
                filtered_scope(b, skip, out);
                out.push("endblock".into());
            }
            S::If {
                condition,
                accept,
                reject,
            } => {
                out.push(format!("if{condition:?}"));
                filtered_scope(accept, skip, out);
                out.push("else".into());
                filtered_scope(reject, skip, out);
                out.push("endif".into());
            }
            S::Loop {
                body,
                continuing,
                break_if,
            } => {
                out.push(format!("loop{break_if:?}"));
                filtered_scope(body, skip, out);
                out.push("continuing".into());
                filtered_scope(continuing, skip, out);
                out.push("endloop".into());
            }
            S::Switch { selector, cases } => {
                out.push(format!("switch{selector:?}"));
                for c in cases {
                    out.push(format!("case{:?}:{}", c.value, c.fall_through));
                    filtered_scope(&c.body, skip, out);
                }
                out.push("endswitch".into());
            }
            _ => out.push(format!("{s:?}")),
        }
    }
}
fn fix_emits(block: &mut ir::Block, skip: &BTreeSet<usize>) {
    let old = std::mem::take(block);
    let mut out = ir::Block::new();
    for (mut s, span) in old.span_into_iter() {
        match &mut s {
            S::Emit(r) => {
                let mut start = None;
                let mut last = None;
                for h in r.clone() {
                    if skip.contains(&h.index()) {
                        if let (Some(a), Some(b)) = (start.take(), last.take()) {
                            out.push(S::Emit(ir::Range::new_from_bounds(a, b)), span);
                        }
                    } else {
                        if start.is_none() {
                            start = Some(h);
                        }
                        last = Some(h);
                    }
                }
                if let (Some(a), Some(b)) = (start, last) {
                    out.push(S::Emit(ir::Range::new_from_bounds(a, b)), span);
                }
                continue;
            }
            S::Block(b) => fix_emits(b, skip),
            S::If { accept, reject, .. } => {
                fix_emits(accept, skip);
                fix_emits(reject, skip);
            }
            S::Loop {
                body, continuing, ..
            } => {
                fix_emits(body, skip);
                fix_emits(continuing, skip);
            }
            S::Switch { cases, .. } => {
                for c in cases {
                    fix_emits(&mut c.body, skip);
                }
            }
            _ => {}
        }
        out.push(s, span);
    }
    *block = out;
}
#[cfg_attr(not(test), allow(dead_code))]
fn projected(
    s: &Sources,
    w: &SceneWitness,
) -> Result<(iridia::Program, Vec<ProjectionSite>), ShaderError> {
    w.validate()?;
    projected_source(s)
}
/// The projection itself: original sources only. The witness only gates
/// [`projected`]; no witness byte enters the program.
fn projected_source(s: &Sources) -> Result<(iridia::Program, Vec<ProjectionSite>), ShaderError> {
    let source = original_samples(s)?;
    let mut p = source_program(s)?;
    let mut global = BTreeMap::new();
    for r in &p.interface.resources {
        let d = r
            .provenance
            .d3d
            .ok_or_else(|| bad("all original resource provenance"))?;
        if d.space != 0 || r.count != 1 {
            return Err(bad("resource array/space outside closed source"));
        }
        let g = p
            .global(r.id)
            .ok_or_else(|| bad("actual global for resource"))?;
        global.insert(g.index(), (d.class, d.register));
    }
    // The original has no sampled function arguments/forwarded texture aliases.
    for (_, f) in p.module.functions.iter() {
        if f.expressions.iter().any(|(_, e)| {
            matches!(
                e,
                E::ImageSample { .. } | E::ImageLoad { .. } | E::ImageQuery { .. }
            )
        }) {
            return Err(bad("foreign sampled helper escape"));
        }
    }
    let f = &p.module.entry_points[0].function;
    let mut samples = vec![];
    let image_reg =
        |f: &ir::Function, h: ir::Handle<E>| -> Result<(iridia::D3dClass, u32), ShaderError> {
            let E::GlobalVariable(g) = f.expressions[h] else {
                return Err(bad("direct source image/sampler handle closure"));
            };
            global
                .get(&g.index())
                .copied()
                .ok_or_else(|| bad("original handle provenance"))
        };
    for (h, e) in f.expressions.iter() {
        match e {
            E::ImageSample {
                image,
                sampler,
                depth_ref,
                level,
                gather,
                clamp_to_edge,
                ..
            } => {
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
    let before = f.clone();
    let mut edits = vec![];
    for ((h, slot, sampler, op), src) in samples.into_iter().zip(&source) {
        if (slot, sampler, op) != (src.slot, src.sampler, src.operation.as_str()) {
            return Err(bad("original sample/slot/sampler exact ordered projection"));
        }
        if !(src.unobserved || src.matched_null) {
            continue;
        }
        let E::ImageSample { image, .. } = before.expressions[h] else {
            unreachable!()
        };
        let E::GlobalVariable(g) = before.expressions[image] else {
            unreachable!()
        };
        let depth = matches!(
            p.module.types[p.module.global_variables[g].ty].inner,
            ir::TypeInner::Image {
                class: ir::ImageClass::Depth { .. },
                ..
            }
        );
        let wanted = if depth {
            ir::TypeInner::Scalar(ir::Scalar::F32)
        } else {
            ir::TypeInner::Vector {
                size: ir::VectorSize::Quad,
                scalar: ir::Scalar::F32,
            }
        };
        let existing = p
            .module
            .types
            .iter()
            .find_map(|(t, v)| (v.inner == wanted).then_some(t));
        // Naga may resolve a scalar/vector sample result as a value type with
        // no existing arena handle. Declare that EXACT type for ZeroValue;
        // never change an old type or any resource/pointer declaration.
        let new_result_type_declaration = existing.is_none();
        let ty = existing.unwrap_or_else(|| {
            p.module.types.insert(
                ir::Type {
                    name: None,
                    inner: wanted,
                },
                ir::Span::UNDEFINED,
            )
        });
        edits.push((
            h,
            ty,
            ProjectionSite {
                expression: h.index(),
                source_word: src.word,
                slot,
                kind: if src.unobserved {
                    "UNOBSERVED"
                } else {
                    "MATCHING_NULL"
                },
                zero_type: ty.index(),
                new_result_type_declaration,
            },
        ));
    }
    if edits.len() != 71 {
        return Err(bad("exact71 typed result projection"));
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
    p.validate()
        .map_err(|e| bad(format!("projected IR validation: {e:?}")))?;
    Ok((p, edits.into_iter().map(|(_, _, r)| r).collect()))
}

fn contract(p: &iridia::Program) -> Result<rc::CallerContract, ShaderError> {
    use rc::{RegisterClass as C, ResourceContractKind as K, UnreadReason as U};
    let mut rows = vec![];
    for r in &p.interface.resources {
        let d = r.provenance.d3d.ok_or_else(|| bad("original binding"))?;
        let (class, binding, resource) = match d.class {
            iridia::D3dClass::ConstantBuffer => (
                C::ConstantBuffer,
                d.register,
                K::ConstantBuffer { bound_bytes: 1536 },
            ),
            iridia::D3dClass::ShaderResource => {
                let k = if NULL.contains(&d.register) || UNOBS.contains(&d.register) {
                    K::UnreadDeclaration {
                        original_resource_sha256: rc::source_declaration_sha256(r).map_err(bad)?,
                        reason: if UNOBS.contains(&d.register) {
                            U::UnobservedSourceBranch
                        } else {
                            U::MatchedNull
                        },
                    }
                } else {
                    match d.register {
                        4 => K::StructuredBuffer {
                            stride_bytes: 4,
                            view: rc::BufferView {
                                source_first_element: 0,
                                source_num_elements: 1,
                                source_stride_bytes: 4,
                                source_resource_bytes: 4,
                            },
                        },
                        5 => K::StructuredBuffer {
                            stride_bytes: 1024,
                            view: rc::BufferView {
                                source_first_element: 0,
                                source_num_elements: 1,
                                source_stride_bytes: 1024,
                                source_resource_bytes: 1024,
                            },
                        },
                        8 => K::TypedBuffer {
                            format: "R32G32B32A32_FLOAT".into(),
                            view: rc::BufferView {
                                source_first_element: 0,
                                source_num_elements: 10,
                                source_stride_bytes: 16,
                                source_resource_bytes: 160,
                            },
                        },
                        34 => K::Texture {
                            dimension: "cube".into(),
                            arrayed: false,
                            multisampled: false,
                            sample: "float".into(),
                        },
                        35..=37 => K::Texture {
                            dimension: "d2".into(),
                            arrayed: false,
                            multisampled: false,
                            sample: "float".into(),
                        },
                        _ => return Err(bad("unadmitted real image/buffer")),
                    }
                };
                (C::ShaderResource, 32 + d.register, k)
            }
            iridia::D3dClass::Sampler => {
                let k = if [3, 4, 6, 7].contains(&d.register) {
                    K::UnreadDeclaration {
                        original_resource_sha256: rc::source_declaration_sha256(r).map_err(bad)?,
                        reason: U::UnusedProjectionDependency,
                    }
                } else if [0, 1].contains(&d.register) {
                    K::Sampler { comparison: false }
                } else {
                    return Err(bad("unknown original sampler"));
                };
                (C::Sampler, 128 + d.register, k)
            }
            _ => return Err(bad("unexpected terrain UAV")),
        };
        rows.push(rc::ResourceContract {
            class,
            register: d.register,
            space: 0,
            count: 1,
            group: 1,
            binding,
            resource,
        });
    }
    Ok(rc::CallerContract {
        revision: rc::RESOURCE_REVISION.into(),
        resources: rows,
    })
}

#[derive(Serialize)]
pub struct PreparedPixel {
    pub revision: &'static str,
    pub preparation_key: String,
    pub wgsl: String,
    pub wgsl_sha256: String,
    pub bindings: Value,
    pub source_scope: Value,
    pub projection: Value,
    pub linking: Value,
    pub source_math_control_preserved: bool,
    pub physical_null_substitutes: bool,
    pub current_world_or_gpu_authority: bool,
}
pub use witness_absent::{SceneWitness, load_manifest, prepare_pixel, validate_pixel};
/// Build without the Scene40 witness development check: the witness type has no values.
mod witness_absent {
    use super::*;
    pub enum SceneWitness {}
    pub fn load_manifest(_: &Sources, _: &Path) -> Result<SceneWitness, ShaderError> {
        Err(bad("the Scene40 witness development check is not part of this build"))
    }
    pub fn prepare_pixel(_: &Sources, w: &SceneWitness) -> Result<PreparedPixel, ShaderError> {
        match *w {}
    }
    pub fn validate_pixel(_: &Sources, w: &SceneWitness, _: &PreparedPixel) -> Result<(), ShaderError> {
        match *w {}
    }
    impl SceneWitness {
        pub(crate) fn validate(&self) -> Result<(), ShaderError> {
            match *self {}
        }
        pub(crate) fn receipt(&self) -> Value {
            match *self {}
        }
    }
}
/// `source_scope` of a preparation made without the Scene40 witness. The
/// projection constants (NULL / unobserved slot sets, bit16-off predicate)
/// are code constants; the bit16-off predicate is checked on the drawn banks
/// by the caller.
pub fn source_only_scope() -> Value {
    json!({"revision":REVISION,"witness":null,
    "source_null_capabilities":source_null_capabilities(),
    "bank_predicate_checked_by_caller":"PSb2[20].x bit16 off on every drawn bank",
    "scope":"original Terrain DXBC from the installed game only; no Scene40 witness read",
    "current_World_or_GPU_authority":false})
}
/// Terrain pixel preparation from the original sources alone.
pub fn prepare_pixel_from_sources(s: &Sources) -> Result<PreparedPixel, ShaderError> {
    let (mut p, sites) = projected_source(s)?;
    let c = contract(&p)?;
    let original_notes = p.notes.clone();
    // The generic resource adapter may bind declarations and classify notes;
    // it must not rewrite any Terrain function expression or statement.
    let function_bodies = format!("{:?} {:?}", p.module.functions, p.module.entry_points);
    let manifest = rc::apply_resources(&mut p, &c).map_err(bad)?;
    if function_bodies != format!("{:?} {:?}", p.module.functions, p.module.entry_points) {
        return Err(bad("resource contract changed Terrain function bodies"));
    }
    if manifest
        .resources
        .iter()
        .filter(|r| r.physical_binding_required)
        .count()
        != 15
    {
        return Err(bad("exact15 physically required PS bindings"));
    }
    p.validate()
        .map_err(|e| bad(format!("bound pixel validation: {e:?}")))?;
    let unresolved = p
        .notes
        .iter()
        .filter(|note| {
            !manifest
                .resolved_notes
                .iter()
                .any(|r| &r.original_note == *note)
        })
        .collect::<Vec<_>>();
    if !unresolved.is_empty() {
        return Err(bad(format!(
            "unresolved pixel lifter notes: {unresolved:?}"
        )));
    }
    // Location-only linkage from original D3D register provenance to unchanged
    // canonical vertex outputs. No padding, value conversion or body edits.
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
            let d = v
                .provenance
                .d3d
                .ok_or_else(|| bad("source varying provenance"))?;
            if d.register == 0
                || d.register > 5
                || v.value.scalar != iridia::ScalarType::Float
                || v.value.width != d2r_shader::terrain::WIDTHS[d.register as usize]
            {
                return Err(bad("exact-width original PS/402 interface"));
            }
            linked.push(json!({"semantic":v.label,"old_location":*location,"original_register_location":d.register,"width":v.value.width,"used":v.used}));
            *location = d.register;
        }
    }
    let info = ir::valid::Validator::new(
        ir::valid::ValidationFlags::all(),
        ir::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| bad(e.to_string()))?;
    let wgsl = ir::back::wgsl::write_string(&module, &info, ir::back::wgsl::WriterFlags::empty())
        .map_err(|e| bad(e.to_string()))?;
    let stock = naga::front::wgsl::parse_str(&wgsl).map_err(|e| bad(e.emit_to_string(&wgsl)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&stock)
    .map_err(|e| bad(e.to_string()))?;
    let preparation_key=sha256(&serde_json::to_vec(&json!({"revision":REVISION,
        "original_sources":s.canonical.identities(),"locked_packages_sha256":sha256(include_bytes!("../Cargo.lock")),
        "sampling51":sample_lod::SAMPLE_LOD_REVISION,"resources52":rc::RESOURCE_REVISION,
        "null_capabilities":source_null_capabilities(),"actual_bank_predicate":"PSb2[20].x==2/bit16off",
        "static_bindings":manifest,"wgsl_sha256":sha256(wgsl.as_bytes()),
        "source_bank45_sha256":SOURCE_BANK45_SHA256,
        "dynamic_frame_banks_lights_images_not_shader_key":true})).map_err(|e|bad(e.to_string()))?);
    Ok(PreparedPixel {
        revision: REVISION,
        preparation_key,
        wgsl_sha256: sha256(wgsl.as_bytes()),
        wgsl,
        bindings: serde_json::to_value(manifest).map_err(|e| bad(e.to_string()))?,
        source_scope: source_only_scope(),
        projection: json!({"new_constant_result_type_ids":sites.iter().filter(|s|s.new_result_type_declaration).map(|s|s.zero_type).collect::<Vec<_>>(),"sites":sites,"source_samples":75,"matched_null":63,"unobserved":8,"control_stores_other_expressions_exact":true,"only_Emit_scope_splits_skip_preemitted_replacement_literals":true,"all_existing_global_interface_types_retained":true,"original_iridia_notes":original_notes,"remaining_iridia_notes":p.notes,"resource_pass_functions_and_entrypoints_exact":true,"unclassified_notes":[]}),
        linking: json!({"location_only":linked,"padding":false,"math_body_rewritten":false,"original_outputs":"o0/o1float4; no velocityo2"}),
        source_math_control_preserved: true,
        physical_null_substitutes: false,
        current_world_or_gpu_authority: false,
    })
}

/// Shadow variant: t39-physical re-projection (child module; reuses the private walk).
#[path = "terrain_scene_shadow895.rs"]
pub mod shadow895;

