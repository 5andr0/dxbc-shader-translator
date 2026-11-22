//! Exact-width D3D semantic linkage and explicit D3D instance-ID adaptation.
use crate::resource_contract::pipeline_bindings;
use iridia::{Program, Stage, SystemValue};
use iridia_naga as ir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const LINK_REVISION: &str = "d2r-iridia-no-padding-semantic-link52-v1";
pub const DRAW_REVISION: &str = "d2r-iridia-explicit-first-instance52-v1";
/// Primitive topologies a linked pair may be drawn with. Topology is pipeline state, not shader
/// state: the vertex/pixel modules, their semantic linkage and the first-instance transport are the
/// same for every admitted topology, so no shader arithmetic depends on the choice. A triangle strip
/// is admitted for non-indexed and indexed draws alike: the stages see only the vertex/instance IDs
/// (in D3D12 and WebGPU the vertex or index value plus the base vertex), while the strip index
/// format and restart value are host pipeline state outside the modules. The recorded topology is
/// part of the link manifest; the host must create its pipeline with exactly that topology.
pub const TOPOLOGIES: [&str; 2] = ["triangle-list", "triangle-strip"];
pub fn supported_topology(topology: &str) -> bool {
    TOPOLOGIES.contains(&topology)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrawContract {
    pub revision: String,
    pub topology: String,
    pub base_group: u32,
    pub base_binding: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DrawManifest {
    pub revision: String,
    pub group: u32,
    pub binding: u32,
    pub uniform_bytes: u32,
    pub first_instance_offset: u32,
    pub source_body_unchanged: bool,
    pub normalization: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedVarying {
    pub semantic: String,
    pub index: u32,
    pub scalar: String,
    pub lanes: u8,
    pub producer_register: u32,
    pub consumer_register: u32,
    pub required_mask: u8,
    pub interpolation: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkManifest {
    pub revision: String,
    pub topology: String,
    pub consumed: Vec<LinkedVarying>,
    pub padding_allowed: bool,
    pub emitted_notes: Vec<String>,
}
pub(crate) struct LinkedStages {
    pub vertex_wgsl: String,
    pub fragment_wgsl: String,
    pub manifest: LinkManifest,
}

/// Preserve the accepted D3D caller rule: WebGPU instance_index - first_instance.
/// The old entry body is moved unchanged to a normal function, not rewritten.
pub(crate) fn apply_draw_contract(
    p: &mut Program,
    c: &DrawContract,
) -> Result<Option<DrawManifest>, String> {
    let mut candidate = p.clone();
    let result = apply_draw_inner(&mut candidate, c)?;
    *p = candidate;
    Ok(result)
}
fn apply_draw_inner(p: &mut Program, c: &DrawContract) -> Result<Option<DrawManifest>, String> {
    p.precision.validate(&p.module,p.provenance.source_hash.as_deref())?;
    let pre = p.module.clone();
    if c.revision != DRAW_REVISION || !supported_topology(&c.topology) {
        return Err("unsupported draw revision/topology".into());
    }
    if p.stage != Stage::Vertex {
        return Err("draw normalization requires vertex program".into());
    }
    if !p
        .interface
        .inputs
        .iter()
        .any(|v| v.system == Some(SystemValue::InstanceIndex))
    {
        return Ok(None);
    }
    if p.module.entry_points.len() != 1 {
        return Err("one original entry point required".into());
    }
    if p.module.global_variables.iter().any(|(_, g)| {
        g.binding
            .as_ref()
            .is_some_and(|b| b.group == c.base_group && b.binding == c.base_binding)
    }) {
        return Err("draw uniform collides with original resource".into());
    }
    let old = p.module.entry_points[0].function.clone();
    let indices = old
        .arguments
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            matches!(
                a.binding,
                Some(ir::Binding::BuiltIn(ir::BuiltIn::InstanceIndex))
            )
        })
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if indices.len() != 1 {
        return Err("exact scalar instance builtin required".into());
    }
    let instance = indices[0];
    if !matches!(
        p.module.types[old.arguments[instance].ty].inner,
        ir::TypeInner::Scalar(ir::Scalar::U32)
    ) {
        return Err("original instance builtin must be u32".into());
    }
    let u4 = p.module.types.insert(
        ir::Type {
            name: Some("D2rDrawBase52".into()),
            inner: ir::TypeInner::Vector {
                size: ir::VectorSize::Quad,
                scalar: ir::Scalar::U32,
            },
        },
        ir::Span::UNDEFINED,
    );
    let uniform_ty = p.module.types.insert(
        ir::Type {
            name: Some("D2rDrawBaseArray52".into()),
            inner: ir::TypeInner::Array {
                base: u4,
                size: ir::ArraySize::Constant(std::num::NonZeroU32::new(1).unwrap()),
                stride: 16,
            },
        },
        ir::Span::UNDEFINED,
    );
    let uniform = p.module.global_variables.append(
        ir::GlobalVariable {
            name: Some("d2r_draw_base52".into()),
            space: ir::AddressSpace::Uniform,
            binding: Some(ir::ResourceBinding {
                group: c.base_group,
                binding: c.base_binding,
            }),
            ty: uniform_ty,
            init: None,
            memory_decorations: Default::default(),
        },
        ir::Span::UNDEFINED,
    );
    let host_id = iridia::ResourceId(
        p.interface
            .resources
            .iter()
            .map(|r| r.id.0)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("host resource id overflow")?,
    );
    p.interface.resources.push(iridia::Resource {
        id: host_id,
        kind: iridia::ResourceKind::ConstantBuffer {
            layout: iridia::BufferLayout {
                size: 16,
                constants: Vec::new(),
            },
        },
        count: 1,
        provenance: iridia::ResourceProvenance {
            name: "D2R explicit host first-instance transport52".into(),
            ..Default::default()
        },
    });
    p.bindings.push((host_id, uniform));
    let mut inner = old.clone();
    inner.name = Some("d2r_original_vertex_body52".into());
    for arg in &mut inner.arguments {
        arg.binding = None;
    }
    if let Some(result) = &mut inner.result {
        result.binding = None;
    }
    let inner = p.module.functions.append(inner, ir::Span::UNDEFINED);
    let mut wrapper = ir::Function {
        name: Some(p.entry.clone()),
        arguments: old.arguments,
        result: old.result,
        ..Default::default()
    };
    let mut args = wrapper
        .arguments
        .iter()
        .enumerate()
        .map(|(i, _)| {
            wrapper.expressions.append(
                ir::Expression::FunctionArgument(i as u32),
                ir::Span::UNDEFINED,
            )
        })
        .collect::<Vec<_>>();
    let ptr = wrapper
        .expressions
        .append(ir::Expression::GlobalVariable(uniform), ir::Span::UNDEFINED);
    let start = wrapper.expressions.len();
    let ptr = wrapper.expressions.append(
        ir::Expression::AccessIndex {
            base: ptr,
            index: 0,
        },
        ir::Span::UNDEFINED,
    );
    let v = wrapper
        .expressions
        .append(ir::Expression::Load { pointer: ptr }, ir::Span::UNDEFINED);
    let base = wrapper.expressions.append(
        ir::Expression::AccessIndex { base: v, index: 0 },
        ir::Span::UNDEFINED,
    );
    let value = wrapper.expressions.append(
        ir::Expression::Binary {
            op: ir::BinaryOperator::Subtract,
            left: args[instance],
            right: base,
        },
        ir::Span::UNDEFINED,
    );
    args[instance] = value;
    wrapper.body.push(
        ir::Statement::Emit(ir::Range::from_index_range(
            start as u32..wrapper.expressions.len() as u32,
            &wrapper.expressions,
        )),
        ir::Span::UNDEFINED,
    );
    let result = wrapper.result.as_ref().map(|_| {
        wrapper
            .expressions
            .append(ir::Expression::CallResult(inner), ir::Span::UNDEFINED)
    });
    wrapper.body.push(
        ir::Statement::Call {
            function: inner,
            arguments: args,
            result,
        },
        ir::Span::UNDEFINED,
    );
    wrapper
        .body
        .push(ir::Statement::Return { value: result }, ir::Span::UNDEFINED);
    p.module.entry_points[0].function = wrapper;
    p.precision = p.precision.relocate_vertex_body(&pre, &p.module, 0, inner)?;
    Ok(Some(DrawManifest {
        revision: DRAW_REVISION.into(),
        group: c.base_group,
        binding: c.base_binding,
        uniform_bytes: 16,
        first_instance_offset: 0,
        source_body_unchanged: true,
        normalization: "wrapping-u32(instance_index-first_instance)".into(),
    }))
}
pub(crate) fn link_pair(
    vs: &Program,
    ps: &Program,
    topology: &str,
) -> Result<LinkedStages, String> {
    let consumed = check_link(vs,ps,topology)?;
    let v = iridia::emit::wgsl_for(
        vs,
        &iridia::emit::Pipeline {
            bindings: pipeline_bindings(vs),
            feeds: Some(&ps.interface),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let p = iridia::emit::wgsl_for(
        ps,
        &iridia::emit::Pipeline {
            bindings: pipeline_bindings(ps),
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let mut notes = v.notes;
    notes.extend(p.notes);
    if !notes.is_empty() {
        return Err(format!("unapproved emitter transformations: {notes:?}"));
    }
    Ok(LinkedStages {
        vertex_wgsl: v.text,
        fragment_wgsl: p.text,
        manifest: LinkManifest {
            revision: LINK_REVISION.into(),
            topology: topology.into(),
            consumed,
            padding_allowed: false,
            emitted_notes: notes,
        },
    })
}

pub(crate) fn check_link(vs:&Program,ps:&Program,topology:&str)->Result<Vec<LinkedVarying>,String>{
    if vs.stage != Stage::Vertex || ps.stage != Stage::Pixel || !supported_topology(topology) {
        return Err("exact vertex/pixel triangle-list/triangle-strip pair required".into());
    }
    let mut seen = BTreeSet::new();
    let mut consumed = Vec::new();
    for v in ps
        .interface
        .inputs
        .iter()
        .filter(|v| v.used && v.system.is_none())
    {
        let key = (v.label.name.to_ascii_uppercase(), v.label.index);
        if !seen.insert(key.clone()) {
            return Err("duplicate consumed semantic".into());
        }
        let outputs = vs
            .interface
            .outputs
            .iter()
            .filter(|o| {
                o.system.is_none()
                    && o.label.name.eq_ignore_ascii_case(&v.label.name)
                    && o.label.index == v.label.index
            })
            .collect::<Vec<_>>();
        if outputs.len() != 1 {
            return Err(format!(
                "missing/duplicate producer semantic {}{}",
                key.0, key.1
            ));
        }
        let o = outputs[0];
        // A source register bitcast may be legal, but this initial route refuses it.
        // No padding or lane fabrication is allowed to make a pair fit.
        if o.value != v.value {
            return Err(format!(
                "{}{} requires explicit source lane/type adaptation, not padding",
                key.0, key.1
            ));
        }
        let a = o
            .provenance
            .d3d
            .ok_or("producer semantic lacks source register/mask")?;
        let b = v
            .provenance
            .d3d
            .ok_or("consumer semantic lacks source register/mask")?;
        let required = b.mask & b.read_write_mask;
        let written = a.mask & !a.read_write_mask;
        if required & !written != 0 {
            return Err("consumer reads undefined/unwritten producer lanes".into());
        }
        let interpolation = v
            .interp
            .ok_or("consumed pixel varying has no source interpolation")?;
        consumed.push(LinkedVarying {
            semantic: key.0,
            index: key.1,
            scalar: format!("{:?}", v.value.scalar),
            lanes: v.value.width,
            producer_register: a.register,
            consumer_register: b.register,
            required_mask: required,
            interpolation: format!("{:?}", interpolation),
        });
    }
    vs.validate().map_err(|e| format!("vertex: {e:?}"))?;
    ps.validate().map_err(|e| format!("pixel: {e:?}"))?;
    Ok(consumed)
}

/// Payload-preserving counterpart of link_pair, with the same exact source interface checks.
pub(crate) struct LinkedPayloadStages104 {
    pub vertex:crate::payload104::ShaderPayload104,
    pub fragment:crate::payload104::ShaderPayload104,
    pub manifest:LinkManifest,
}
pub(crate) fn link_payload_pair104(vs:&Program,ps:&Program,topology:&str,controlled:bool)->Result<LinkedPayloadStages104,String>{
    if !controlled && !vs.precision.is_precision_bearing() && !ps.precision.is_precision_bearing(){
        let linked=link_pair(vs,ps,topology)?;
        return Ok(LinkedPayloadStages104{vertex:crate::payload104::ShaderPayload104::Wgsl{text:linked.vertex_wgsl.into()},fragment:crate::payload104::ShaderPayload104::Wgsl{text:linked.fragment_wgsl.into()},manifest:linked.manifest})
    }
    let consumed=check_link(vs,ps,topology)?;
    let v=crate::precision_carrier::prepare_spirv(vs,&iridia::emit::Pipeline{bindings:pipeline_bindings(vs),feeds:Some(&ps.interface),..Default::default()})?;
    let p=crate::precision_carrier::prepare_spirv(ps,&iridia::emit::Pipeline{bindings:pipeline_bindings(ps),..Default::default()})?;
    Ok(LinkedPayloadStages104{vertex:crate::payload104::ShaderPayload104::controlled(&v).map_err(|e|e.to_string())?,fragment:crate::payload104::ShaderPayload104::controlled(&p).map_err(|e|e.to_string())?,manifest:LinkManifest{revision:LINK_REVISION.into(),topology:topology.into(),consumed,padding_allowed:false,emitted_notes:Vec::new()}})
}
