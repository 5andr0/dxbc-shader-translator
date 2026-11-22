//! Source-indexed caller bindings for the public Iridia program boundary.
//! This module never supplies an absent resource or infers a view format.
use iridia::{BoundSizes, BufferKind, D3dClass, Program, ResourceId, ResourceKind};
use iridia_naga as ir;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const RESOURCE_REVISION: &str = "d2r-iridia-explicit-resources52-v2";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterClass {
    ConstantBuffer,
    ShaderResource,
    Sampler,
    UnorderedAccess,
}
impl RegisterClass {
    fn source(self) -> D3dClass {
        match self {
            Self::ConstantBuffer => D3dClass::ConstantBuffer,
            Self::ShaderResource => D3dClass::ShaderResource,
            Self::Sampler => D3dClass::Sampler,
            Self::UnorderedAccess => D3dClass::UnorderedAccess,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BufferView {
    pub source_first_element: u64,
    pub source_num_elements: u64,
    pub source_stride_bytes: u64,
    pub source_resource_bytes: u64,
}
impl BufferView {
    /// Exact standalone range the host must bind, without padding or expansion.
    pub fn range(&self) -> Result<std::ops::Range<u64>, String> {
        if self.source_stride_bytes == 0 || self.source_num_elements == 0 {
            return Err("zero buffer stride/count".into());
        }
        let start = self
            .source_first_element
            .checked_mul(self.source_stride_bytes)
            .ok_or("view base overflow")?;
        let len = self
            .source_num_elements
            .checked_mul(self.source_stride_bytes)
            .ok_or("view length overflow")?;
        let end = start.checked_add(len).ok_or("view end overflow")?;
        if end > self.source_resource_bytes || len > u32::MAX as u64 {
            return Err("buffer view exceeds source/bounded shader indexing".into());
        }
        Ok(start..end)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResourceContractKind {
    ConstantBuffer {
        bound_bytes: u32,
    },
    RawBuffer {
        view: BufferView,
    },
    StructuredBuffer {
        stride_bytes: u32,
        view: BufferView,
    },
    /// Only a full four-lane 32-bit format is admitted in this first pass.
    /// Narrow/normalized formats need an explicit IR conversion, never padding.
    TypedBuffer {
        format: String,
        view: BufferView,
    },
    Texture {
        dimension: String,
        arrayed: bool,
        multisampled: bool,
        sample: String,
    },
    Sampler {
        comparison: bool,
    },
    StorageBuffer {
        access: String,
        stride_bytes: u32,
        view: BufferView,
    },
    StorageImage {
        dimension: String,
        arrayed: bool,
        access: String,
        format: String,
    },
    /// A retained original declaration, proved entrypoint-unread after a
    /// separately admitted source/caller projection. This is not a texture view.
    UnreadDeclaration {
        original_resource_sha256: String,
        reason: UnreadReason,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnreadReason {
    MatchedNull,
    UnobservedSourceBranch,
    UnusedProjectionDependency,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceContract {
    pub class: RegisterClass,
    pub register: u32,
    pub space: u32,
    pub count: u32,
    pub group: u32,
    pub binding: u32,
    pub resource: ResourceContractKind,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallerContract {
    pub revision: String,
    pub resources: Vec<ResourceContract>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingManifestEntry {
    pub source_resource_id: u32,
    pub class: RegisterClass,
    pub register: u32,
    pub space: u32,
    pub group: u32,
    pub binding: u32,
    pub resource: ResourceContractKind,
    pub physical_binding_required: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingManifest {
    pub revision: String,
    pub resources: Vec<BindingManifestEntry>,
    pub standalone_exact_view_rebase_required: bool,
    pub host_resource_presence_proved: bool,
    pub resolved_notes: Vec<ResolvedNote>,
    pub storage_formats: Vec<StorageFormatChange>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageFormatChange {
    pub resource_id: u32,
    pub original_ir_format: String,
    pub actual_view_format: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedNote {
    pub original_note: String,
    pub rule: String,
    pub source_resource_ids: Vec<u32>,
    pub source_and_transport_bits: String,
}
const TYPED_NOTE: &str = "a typed buffer view is read as four 32-bit lanes per element: converting from the view's format is the binding's";
const STORAGE_NOTE: &str = "a typed unordered-access view is carried as a storage image of four 32-bit lanes: its format is the binding's";
/// Digest of the lifted source declaration (including authorized bound CB size),
/// not an image payload hash or a stand-in for the original DXBC identity.
pub(crate) fn source_declaration_sha256(r: &iridia::Resource) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(r).map_err(|e| e.to_string())?)
    ))
}

fn joined<'a>(
    program: &'a Program,
    contract: &'a CallerContract,
) -> Result<Vec<(ResourceId, &'a ResourceContract)>, String> {
    if contract.revision != RESOURCE_REVISION {
        return Err("unknown resource contract revision".into());
    }
    if program.interface.resources.len() != contract.resources.len() {
        return Err("one explicit contract per declared resource required".into());
    }
    let mut keys = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for c in &contract.resources {
        if c.count != 1 || c.space != 0 {
            return Err("resource arrays/spaces require separate supported contract".into());
        }
        if !keys.insert((c.class, c.space, c.register))
            || !destinations.insert((c.group, c.binding))
        {
            return Err("duplicate source register or destination binding".into());
        }
    }
    let mut rows = Vec::new();
    for r in &program.interface.resources {
        let d = r
            .provenance
            .d3d
            .ok_or("source resource has no D3D register")?;
        if r.count != 1 {
            return Err("source resource array unsupported".into());
        }
        let c = contract
            .resources
            .iter()
            .find(|c| c.class.source() == d.class && c.space == d.space && c.register == d.register)
            .ok_or_else(|| {
                format!(
                    "missing explicit contract for {:?}{} space{}",
                    d.class, d.register, d.space
                )
            })?;
        rows.push((r.id, c));
    }
    Ok(rows)
}
/// Run after an initial declaration lift, then re-lift with the returned sizes.
pub(crate) fn prepare_contract(
    program: &Program,
    contract: &CallerContract,
) -> Result<BoundSizes, String> {
    let mut sizes = BoundSizes::new();
    for (id, c) in joined(program, contract)? {
        let r = program
            .interface
            .resources
            .iter()
            .find(|r| r.id == id)
            .ok_or("resource id")?;
        if let ResourceContractKind::ConstantBuffer { bound_bytes } = c.resource {
            let ResourceKind::ConstantBuffer { ref layout } = r.kind else {
                return Err("CB contract for non-CB declaration".into());
            };
            if bound_bytes < layout.size || bound_bytes % 16 != 0 || bound_bytes > 65536 {
                return Err(
                    "bound CB extent smaller than declaration, unaligned or exceeds D3D64KiB"
                        .into(),
                );
            }
            sizes.set(id, bound_bytes);
        }
    }
    Ok(sizes)
}
fn sample_name(s: iridia::SampleType) -> &'static str {
    match s {
        iridia::SampleType::Float => "float",
        iridia::SampleType::Uint => "uint",
        iridia::SampleType::Sint => "sint",
        iridia::SampleType::Depth => "depth",
    }
}
fn dim_name(d: iridia::TextureDim) -> &'static str {
    match d {
        iridia::TextureDim::D1 => "d1",
        iridia::TextureDim::D2 => "d2",
        iridia::TextureDim::D3 => "d3",
        iridia::TextureDim::Cube => "cube",
    }
}
/// Validate the actual caller declaration and set only its binding location.
pub(crate) fn apply_resources(
    program: &mut Program,
    contract: &CallerContract,
) -> Result<BindingManifest, String> {
    let mut candidate = program.clone();
    let result = apply_resources_inner(&mut candidate, contract)?;
    *program = candidate;
    Ok(result)
}
fn apply_resources_inner(program: &mut Program, contract: &CallerContract) -> Result<BindingManifest,String> {
    let pre = program.module.clone();
    let rows = joined(program, contract)?
        .into_iter()
        .map(|(id, c)| (id, c.clone()))
        .collect::<Vec<_>>();
    let mut manifests = Vec::new();
    let mut changed_types = Vec::new();
    let mut storage_formats = Vec::new();
    let mut typed_buffers = Vec::new();
    let mut storage_images = Vec::new();
    let unread = if rows
        .iter()
        .any(|(_, c)| matches!(c.resource, ResourceContractKind::UnreadDeclaration { .. }))
    {
        Some(
            program
                .unread_resources()
                .map_err(|e| format!("unread declaration analysis: {e:?}"))?,
        )
    } else {
        None
    };
    for (id, c) in rows {
        let r = program
            .interface
            .resources
            .iter()
            .find(|r| r.id == id)
            .ok_or("resource id")?;
        let global = program
            .global(id)
            .ok_or("source binding has no module global")?;
        let ty = program.module.global_variables[global].ty;
        let physical_binding_required =
            !matches!(c.resource, ResourceContractKind::UnreadDeclaration { .. });
        match (&r.kind, &c.resource) {
            (
                _,
                ResourceContractKind::UnreadDeclaration {
                    original_resource_sha256,
                    ..
                },
            ) => {
                if !unread.as_ref().is_some_and(|set| set.contains(&id)) {
                    return Err(
                        "declaration-only resource is still reachable by original entrypoint"
                            .into(),
                    );
                }
                let actual = source_declaration_sha256(r)?;
                if &actual != original_resource_sha256 {
                    return Err("declaration-only original resource hash differs".into());
                }
            }
            (
                ResourceKind::ConstantBuffer { layout },
                ResourceContractKind::ConstantBuffer { bound_bytes },
            ) => {
                if layout.size != *bound_bytes {
                    return Err("lift_with actual bound CB extent required before apply".into());
                }
                if !matches!(program.module.types[ty].inner,ir::TypeInner::Array{size:ir::ArraySize::Constant(n),stride:16,..} if n.get().checked_mul(16)==Some(*bound_bytes))
                {
                    return Err("CB IR extent differs from explicit bound size".into());
                }
            }
            (ResourceKind::Buffer(BufferKind::Raw), ResourceContractKind::RawBuffer { view }) => {
                view.range()?;
                if view.source_stride_bytes != 4 {
                    return Err("raw source view must use 4-byte words".into());
                }
                raw_array(program, ty)?;
            }
            (
                ResourceKind::Buffer(BufferKind::Structured { stride }),
                ResourceContractKind::StructuredBuffer { stride_bytes, view },
            ) => {
                view.range()?;
                if stride != stride_bytes
                    || u64::from(*stride) != view.source_stride_bytes
                    || *stride == 0
                    || stride % 4 != 0
                {
                    return Err("structured view stride differs from original declaration".into());
                }
                raw_array(program, ty)?;
            }
            (
                ResourceKind::Buffer(BufferKind::Typed(v)),
                ResourceContractKind::TypedBuffer { format, view },
            ) => {
                view.range()?;
                let expected = match v.scalar {
                    iridia::ScalarType::Float => "R32G32B32A32_FLOAT",
                    iridia::ScalarType::Uint => "R32G32B32A32_UINT",
                    iridia::ScalarType::Sint => "R32G32B32A32_SINT",
                    _ => return Err("unsupported typed scalar".into()),
                };
                if v.width != 4 || format != expected || view.source_stride_bytes != 16 {
                    return Err(
                        "typed view requires source-preserving format lowering not yet supplied"
                            .into(),
                    );
                }
                if !matches!(program.module.types[ty].inner,ir::TypeInner::Array{base,size:ir::ArraySize::Dynamic,stride:16} if matches!(program.module.types[base].inner,ir::TypeInner::Vector{size:ir::VectorSize::Quad,scalar:ir::Scalar::U32}))
                {
                    return Err("typed buffer IR layout mismatch".into());
                }
                typed_buffers.push(id.0);
            }
            (
                ResourceKind::Texture {
                    dim,
                    arrayed,
                    multisampled,
                    sample,
                },
                ResourceContractKind::Texture {
                    dimension,
                    arrayed: a,
                    multisampled: m,
                    sample: s,
                },
            ) => {
                if dim_name(*dim) != dimension
                    || arrayed != a
                    || multisampled != m
                    || sample_name(*sample) != s
                {
                    return Err(
                        "texture caller view differs from source dimensionality/sample kind".into(),
                    );
                }
            }
            (
                ResourceKind::Sampler { comparison },
                ResourceContractKind::Sampler { comparison: c },
            ) if comparison == c => {}
            (
                ResourceKind::Storage {
                    target: iridia::StorageTarget::Buffer(buffer),
                    access,
                },
                ResourceContractKind::StorageBuffer {
                    access: a,
                    stride_bytes,
                    view,
                },
            ) => {
                if access_name(*access) != a {
                    return Err("storage buffer access differs from source".into());
                }
                let stride = match buffer {
                    BufferKind::Raw => 4,
                    BufferKind::Structured { stride } => *stride,
                    _ => return Err("typed UAV buffer needs separate view lowering".into()),
                };
                if stride != *stride_bytes
                    || stride == 0
                    || stride % 4 != 0
                    || u64::from(stride) != view.source_stride_bytes
                {
                    return Err("storage buffer stride differs from source".into());
                }
                view.range()?;
                if !matches!(program.module.types[ty].inner,ir::TypeInner::Array{base,size:ir::ArraySize::Dynamic,stride:4} if matches!(program.module.types[base].inner,ir::TypeInner::Scalar(ir::Scalar::U32)|ir::TypeInner::Atomic(ir::Scalar::U32)))
                {
                    return Err("storage word/atomic buffer IR layout mismatch".into());
                }
            }
            (
                ResourceKind::Storage {
                    target:
                        iridia::StorageTarget::Image {
                            dim,
                            arrayed,
                            sample,
                        },
                    access,
                },
                ResourceContractKind::StorageImage {
                    dimension,
                    arrayed: a,
                    access: ac,
                    format,
                },
            ) => {
                if dim_name(*dim) != dimension || arrayed != a || access_name(*access) != ac {
                    return Err("storage image dimension/access differs from source".into());
                }
                let current = program.module.types[ty].inner.clone();
                let ir::TypeInner::Image {
                    dim: nd,
                    arrayed: na,
                    class:
                        ir::ImageClass::Storage {
                            format: old,
                            access: permissions,
                        },
                } = current
                else {
                    return Err("source UAV image has unexpected IR class".into());
                };
                let (new, scalar) = storage_format(format)?;
                if sample_name(*sample) != scalar {
                    return Err("storage format changes source sample scalar kind".into());
                }
                changed_types.push((
                    global,
                    ir::Type {
                        name: None,
                        inner: ir::TypeInner::Image {
                            dim: nd,
                            arrayed: na,
                            class: ir::ImageClass::Storage {
                                format: new,
                                access: permissions,
                            },
                        },
                    },
                ));
                storage_formats.push(StorageFormatChange {
                    resource_id: id.0,
                    original_ir_format: format!("{old:?}"),
                    actual_view_format: format.clone(),
                });
                storage_images.push(id.0);
            }
            _ => {
                return Err(format!(
                    "unsupported/mismatched source resource {:?}",
                    r.kind
                ));
            }
        }
        manifests.push(BindingManifestEntry {
            source_resource_id: id.0,
            class: c.class,
            register: c.register,
            space: c.space,
            group: c.group,
            binding: c.binding,
            resource: c.resource,
            physical_binding_required,
        });
    }
    // Mutate only after every original declaration and caller contract passed.
    for (global, ty) in changed_types {
        program.module.global_variables[global].ty =
            program.module.types.insert(ty, ir::Span::UNDEFINED);
    }
    for m in &manifests {
        let global = program
            .global(ResourceId(m.source_resource_id))
            .ok_or("validated global disappeared")?;
        program.module.global_variables[global].binding = Some(ir::ResourceBinding {
            group: m.group,
            binding: m.binding,
        });
    }
    manifests.sort_by_key(|r| (r.group, r.binding));
    let mut resolved_notes = Vec::new();
    program.notes.retain(|n| {
        if n==TYPED_NOTE&&!typed_buffers.is_empty(){
            resolved_notes.push(ResolvedNote{original_note:n.clone(),rule:"exact-unnormalized-four-32bit-lane-view".into(),source_resource_ids:typed_buffers.clone(),source_and_transport_bits:"128-bit original view element = unchanged vec4<u32> storage element; original DXBC numeric consumers retain their bitcasts; no expansion or normalization".into()}); false
        }else if n==STORAGE_NOTE&&!storage_images.is_empty(){
            resolved_notes.push(ResolvedNote{original_note:n.clone(),rule:"explicit-actual-uav-format-declaration".into(),source_resource_ids:storage_images.clone(),source_and_transport_bits:"same image operations/values/coordinates/access; per-global storage type cloned to caller's actual format; original and selected formats recorded".into()});false
        }else{true}
    });
    program.precision = program.precision.unchanged_arithmetic(&pre,&program.module,iridia::precision::TransformKind::ResourceBindings)?;
    Ok(BindingManifest {
        revision: RESOURCE_REVISION.into(),
        resources: manifests,
        standalone_exact_view_rebase_required: true,
        host_resource_presence_proved: false,
        resolved_notes,
        storage_formats,
    })
}
fn access_name(a: iridia::Access) -> &'static str {
    match a {
        iridia::Access::Read => "read",
        iridia::Access::Write => "write",
        iridia::Access::ReadWrite => "read_write",
    }
}
fn storage_format(s: &str) -> Result<(ir::StorageFormat, &'static str), String> {
    Ok(match s {
        "R32Float" => (ir::StorageFormat::R32Float, "float"),
        "Rgba16Float" => (ir::StorageFormat::Rgba16Float, "float"),
        // Preintegrated Skin lookup (skin_preintegrate_c u4): DXGI format 28 = R8G8B8A8_UNORM.
        "Rgba8Unorm" => (ir::StorageFormat::Rgba8Unorm, "float"),
        "Rgba32Float" => (ir::StorageFormat::Rgba32Float, "float"),
        "R32Uint" => (ir::StorageFormat::R32Uint, "uint"),
        "Rgba32Uint" => (ir::StorageFormat::Rgba32Uint, "uint"),
        "R32Sint" => (ir::StorageFormat::R32Sint, "sint"),
        "Rgba32Sint" => (ir::StorageFormat::Rgba32Sint, "sint"),
        _ => return Err("storage view format not explicitly implemented".into()),
    })
}
fn raw_array(program: &Program, ty: ir::Handle<ir::Type>) -> Result<(), String> {
    if matches!(program.module.types[ty].inner,ir::TypeInner::Array{base,size:ir::ArraySize::Dynamic,stride:4} if matches!(program.module.types[base].inner,ir::TypeInner::Scalar(ir::Scalar::U32)))
    {
        Ok(())
    } else {
        Err("raw/structured IR must retain exact u32 word array stride4".into())
    }
}

pub(crate) fn pipeline_bindings(program: &Program) -> BTreeMap<ResourceId, (u32, u32)> {
    program
        .bindings
        .iter()
        .filter_map(|(id, g)| {
            program.module.global_variables[*g]
                .binding
                .map(|b| (*id, (b.group, b.binding)))
        })
        .collect()
}
