use crate::ShaderError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceUsage {
    pub group: u32,
    pub binding: u32,
    pub entrypoint_used: bool,
    pub usage: String,
}
/// Static entrypoint interface usage, including Naga query/named-expression
/// rules. This is not a claim that every pixel fetches a dynamically guarded
/// resource. Legacy artifacts without this metadata have unknown usage.
pub fn entrypoint_resource_usage(
    module: &naga::Module,
    info: &naga::valid::ModuleInfo,
) -> Result<Vec<ResourceUsage>, ShaderError> {
    if module.entry_points.len() != 1 {
        return Err(ShaderError::Unsupported(
            "exact single-entrypoint usage metadata required".into(),
        ));
    }
    let ep = info.get_entry_point(0);
    let mut out = Vec::new();
    for (h, g) in module.global_variables.iter() {
        if let Some(b) = g.binding {
            out.push(ResourceUsage {
                group: b.group,
                binding: b.binding,
                entrypoint_used: !ep[h].is_empty(),
                usage: format!("{:?}", ep[h]),
            });
        }
    }
    out.sort_by_key(|v| (v.group, v.binding));
    Ok(out)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InterfaceSlot {
    pub location: Option<u32>,
    pub builtin: Option<String>,
    pub scalar: String,
    pub width: u8,
    pub components: u32,
    pub interpolation: Option<String>,
    pub sampling: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntryPointLayout {
    pub name: String,
    pub stage: String,
    pub inputs: Vec<InterfaceSlot>,
    pub outputs: Vec<InterfaceSlot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workgroup_size: Option<[u32; 3]>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceLayout {
    pub group: u32,
    pub binding: u32,
    pub name: Option<String>,
    pub kind: String,
    pub minimum_binding_size: Option<u64>,
    pub runtime_array_stride: Option<u32>,
    pub source_view_format: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShaderLayout {
    pub entry_points: Vec<EntryPointLayout>,
    pub resources: Vec<ResourceLayout>,
    pub coordinate_adjustment_applied: bool,
}
fn interface(
    module: &naga::Module,
    ty: naga::Handle<naga::Type>,
    binding: Option<&naga::Binding>,
    out: &mut Vec<InterfaceSlot>,
) -> Result<(), ShaderError> {
    if binding.is_none() {
        if let naga::TypeInner::Struct { members, .. } = &module.types[ty].inner {
            for m in members {
                interface(module, m.ty, m.binding.as_ref(), out)?;
            }
            return Ok(());
        }
    }
    let Some(binding) = binding else {
        return Err(ShaderError::Unsupported(
            "entrypoint value without binding".into(),
        ));
    };
    let (scalar, components) = match module.types[ty].inner {
        naga::TypeInner::Scalar(s) => (s, 1),
        naga::TypeInner::Vector { size, scalar } => (scalar, size as u32),
        _ => {
            return Err(ShaderError::Unsupported(
                "non scalar/vector stage interface".into(),
            ));
        }
    };
    let (location, builtin, interpolation, sampling) = match binding {
        naga::Binding::BuiltIn(b) => (None, Some(format!("{b:?}")), None, None),
        naga::Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        } => (
            Some(*location),
            None,
            interpolation.map(|v| format!("{v:?}")),
            sampling.map(|v| format!("{v:?}")),
        ),
    };
    out.push(InterfaceSlot {
        location,
        builtin,
        scalar: format!("{:?}", scalar.kind),
        width: scalar.width,
        components,
        interpolation,
        sampling,
    });
    Ok(())
}
pub fn reflect(module: &naga::Module) -> Result<ShaderLayout, ShaderError> {
    let mut layout = naga::proc::Layouter::default();
    layout
        .update(module.to_ctx())
        .map_err(|e| ShaderError::Validation(format!("type layout: {e:?}")))?;
    let mut entries = Vec::new();
    for e in &module.entry_points {
        let (mut inputs, mut outputs) = (Vec::new(), Vec::new());
        for a in &e.function.arguments {
            interface(module, a.ty, a.binding.as_ref(), &mut inputs)?;
        }
        if let Some(r) = &e.function.result {
            interface(module, r.ty, r.binding.as_ref(), &mut outputs)?;
        }
        entries.push(EntryPointLayout {
            name: e.name.clone(),
            stage: format!("{:?}", e.stage),
            inputs,
            outputs,
            workgroup_size: (e.stage == naga::ShaderStage::Compute).then_some(e.workgroup_size),
        });
    }
    let mut resources = Vec::new();
    for (_, v) in module.global_variables.iter() {
        let Some(b) = v.binding else {
            continue;
        };
        let mut min = None;
        let mut stride = None;
        let kind = match v.space {
            naga::AddressSpace::Uniform => {
                min = Some(layout[v.ty].size as u64);
                "uniform-buffer".into()
            }
            naga::AddressSpace::Storage { access } => {
                min = Some(layout[v.ty].size as u64);
                if let naga::TypeInner::Struct { ref members, .. } = module.types[v.ty].inner {
                    if let Some(m) = members.last() {
                        if let naga::TypeInner::Array {
                            size: naga::ArraySize::Dynamic,
                            stride: s,
                            ..
                        } = module.types[m.ty].inner
                        {
                            stride = Some(s);
                        }
                    }
                }
                if access.contains(naga::StorageAccess::STORE) {
                    "storage-buffer-read-write".into()
                } else {
                    "storage-buffer-readonly".into()
                }
            }
            naga::AddressSpace::Handle => match module.types[v.ty].inner {
                naga::TypeInner::Sampler { comparison } => {
                    format!("sampler-comparison-{comparison}")
                }
                naga::TypeInner::Image {
                    dim,
                    arrayed,
                    class,
                } => format!("image-{dim:?}-array{arrayed}-{class:?}"),
                _ => {
                    return Err(ShaderError::Unsupported(
                        "unknown bound handle resource".into(),
                    ));
                }
            },
            _ => {
                return Err(ShaderError::Unsupported(
                    "unsupported bound resource address space".into(),
                ));
            }
        };
        resources.push(ResourceLayout {
            group: b.group,
            binding: b.binding,
            name: v.name.clone(),
            kind,
            minimum_binding_size: min,
            runtime_array_stride: stride,
            source_view_format: None,
        });
    }
    resources.sort_by_key(|r| (r.group, r.binding));
    Ok(ShaderLayout {
        entry_points: entries,
        resources,
        coordinate_adjustment_applied: false,
    })
}

pub fn validate_vertex_fragment_pair(
    vs: &ShaderLayout,
    ps: &ShaderLayout,
) -> Result<usize, ShaderError> {
    let v = vs
        .entry_points
        .iter()
        .find(|e| e.stage == "Vertex")
        .ok_or_else(|| ShaderError::Unsupported("vertex entrypoint absent".into()))?;
    let p = ps
        .entry_points
        .iter()
        .find(|e| e.stage == "Fragment")
        .ok_or_else(|| ShaderError::Unsupported("fragment entrypoint absent".into()))?;
    let mut count = 0;
    for input in p.inputs.iter().filter(|i| i.location.is_some()) {
        let output = v
            .outputs
            .iter()
            .find(|o| o.location == input.location)
            .ok_or_else(|| {
                ShaderError::Validation(format!(
                    "missing vertex output location{:?}",
                    input.location
                ))
            })?;
        if output.scalar != input.scalar
            || output.width != input.width
            || output.components != input.components
        {
            return Err(ShaderError::Validation(format!(
                "stage varying type mismatch location{:?}",
                input.location
            )));
        }
        if input.interpolation.as_deref() == Some("Flat")
            && output.interpolation.as_deref() != Some("Flat")
        { /* SPIR-V vertex decoration may omit interpolation. Fragment owns the qualifier. */
        }
        count += 1;
    }
    Ok(count)
}
