//! Stage interface of a SPIR-V payload, read from the module words the backend consumes.
//! Only the entry point's Input/Output variables are reflected: their Location or BuiltIn,
//! component type and interpolation decorations. Debug names are never consulted.
use std::collections::BTreeMap;

const MAGIC: u32 = 0x0723_0203;
const OP_ENTRY_POINT: u16 = 15;
const OP_TYPE_BOOL: u16 = 20;
const OP_TYPE_INT: u16 = 21;
const OP_TYPE_FLOAT: u16 = 22;
const OP_TYPE_VECTOR: u16 = 23;
const OP_TYPE_POINTER: u16 = 32;
const OP_VARIABLE: u16 = 59;
const OP_DECORATE: u16 = 71;
const OP_MEMBER_DECORATE: u16 = 72;
const STORAGE_INPUT: u32 = 1;
const STORAGE_OUTPUT: u32 = 3;
const DECORATION_BUILT_IN: u32 = 11;
const DECORATION_NO_PERSPECTIVE: u32 = 13;
const DECORATION_FLAT: u32 = 14;
const DECORATION_PATCH: u32 = 15;
const DECORATION_CENTROID: u32 = 16;
const DECORATION_SAMPLE: u32 = 17;
const DECORATION_LOCATION: u32 = 30;
const DECORATION_COMPONENT: u32 = 31;
const DECORATION_INDEX: u32 = 32;
const DECORATION_PER_PRIMITIVE: u32 = 5271;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExecutionModel {
    Vertex,
    Fragment,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Bool,
    Float,
    Uint,
    Sint,
}
/// A scalar or vector component type: kind, bit width, lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Component {
    pub kind: Kind,
    pub width: u32,
    pub lanes: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Interpolation {
    Smooth,
    NoPerspective,
    Flat,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Sampling {
    Center,
    Centroid,
    Sample,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Varying {
    pub component: Component,
    pub interpolation: Interpolation,
    pub sampling: Sampling,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Interface {
    pub inputs: BTreeMap<u32, Varying>,
    pub outputs: BTreeMap<u32, Varying>,
    /// SPIR-V BuiltIn values and their component types.
    pub builtin_inputs: BTreeMap<u32, Component>,
    pub builtin_outputs: BTreeMap<u32, Component>,
}
#[derive(Debug)]
pub(crate) struct Reflected {
    pub model: ExecutionModel,
    pub interface: Interface,
}

#[derive(Default)]
struct Decorations {
    location: Option<u32>,
    builtin: Option<u32>,
    flat: bool,
    no_perspective: bool,
    centroid: bool,
    sample: bool,
    unsupported: Option<u32>,
}

/// Reflect the single entry point's stage interface. Refuses what this lineage never emits
/// (several entry points, Component/Index/Patch decorations, interface blocks, arrays).
pub(crate) fn reflect(words: &[u32]) -> Result<Reflected, String> {
    if words.len() < 5 || words[0] != MAGIC {
        return Err("SPIR-V header".into());
    }
    let mut entry: Option<(ExecutionModel, Vec<u32>)> = None;
    let mut entries = 0;
    let mut types: BTreeMap<u32, Component> = BTreeMap::new();
    let mut pointers: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
    let mut variables: BTreeMap<u32, (u32, u32)> = BTreeMap::new();
    let mut decorations: BTreeMap<u32, Decorations> = BTreeMap::new();
    let mut member_decorated = std::collections::BTreeSet::new();
    let mut at = 5;
    while at < words.len() {
        let count = (words[at] >> 16) as usize;
        let op = (words[at] & 0xffff) as u16;
        if count == 0 || at + count > words.len() {
            return Err("SPIR-V instruction extent".into());
        }
        let operands = &words[at + 1..at + count];
        match op {
            OP_ENTRY_POINT => {
                entries += 1;
                let model = match operands.first() {
                    Some(0) => ExecutionModel::Vertex,
                    Some(4) => ExecutionModel::Fragment,
                    _ => return Err("SPIR-V entry point is not a vertex or fragment stage".into()),
                };
                // operands: model, entry id, literal name (nul-terminated, word padded), interface ids
                let name_words = operands
                    .get(2..)
                    .and_then(|rest| rest.iter().position(|w| w.to_le_bytes().contains(&0)))
                    .ok_or("SPIR-V entry point name")?
                    + 1;
                entry = Some((model, operands[2 + name_words..].to_vec()));
            }
            OP_TYPE_BOOL => {
                types.insert(operands[0], Component { kind: Kind::Bool, width: 32, lanes: 1 });
            }
            OP_TYPE_INT if operands.len() == 3 => {
                let kind = if operands[2] == 0 { Kind::Uint } else { Kind::Sint };
                types.insert(operands[0], Component { kind, width: operands[1], lanes: 1 });
            }
            OP_TYPE_FLOAT if operands.len() >= 2 => {
                types.insert(operands[0], Component { kind: Kind::Float, width: operands[1], lanes: 1 });
            }
            OP_TYPE_VECTOR if operands.len() == 3 => {
                if let Some(c) = types.get(&operands[1]).copied() {
                    types.insert(operands[0], Component { lanes: operands[2], ..c });
                }
            }
            OP_TYPE_POINTER if operands.len() == 3 => {
                pointers.insert(operands[0], (operands[1], operands[2]));
            }
            OP_VARIABLE if operands.len() >= 3 => {
                if matches!(operands[2], STORAGE_INPUT | STORAGE_OUTPUT) {
                    variables.insert(operands[1], (operands[0], operands[2]));
                }
            }
            OP_DECORATE if operands.len() >= 2 => {
                let d = decorations.entry(operands[0]).or_default();
                let value = operands.get(2).copied();
                match operands[1] {
                    DECORATION_LOCATION => d.location = value,
                    DECORATION_BUILT_IN => d.builtin = value,
                    DECORATION_FLAT => d.flat = true,
                    DECORATION_NO_PERSPECTIVE => d.no_perspective = true,
                    DECORATION_CENTROID => d.centroid = true,
                    DECORATION_SAMPLE => d.sample = true,
                    x @ (DECORATION_COMPONENT | DECORATION_INDEX | DECORATION_PATCH | DECORATION_PER_PRIMITIVE) => {
                        d.unsupported = Some(x)
                    }
                    _ => {}
                }
            }
            OP_MEMBER_DECORATE if operands.len() >= 3 => {
                if matches!(operands[2], DECORATION_LOCATION | DECORATION_BUILT_IN) {
                    member_decorated.insert(operands[0]);
                }
            }
            _ => {}
        }
        at += count;
    }
    if entries != 1 {
        return Err(format!("SPIR-V payload has {entries} entry points; one is required"));
    }
    let (model, ids) = entry.ok_or("SPIR-V entry point")?;
    let mut interface = Interface::default();
    for id in ids {
        // SPIR-V 1.4 lists every global; only stage inputs/outputs are the interface here
        let Some(&(pointer, storage)) = variables.get(&id) else { continue };
        let &(pointer_storage, pointee) = pointers.get(&pointer).ok_or("SPIR-V interface pointer type")?;
        if pointer_storage != storage {
            return Err("SPIR-V interface pointer storage class".into());
        }
        if member_decorated.contains(&pointee) {
            return Err("SPIR-V interface block; this lineage emits one variable per varying".into());
        }
        let component = *types.get(&pointee).ok_or("SPIR-V interface variable is not a scalar or vector")?;
        let d = decorations.remove(&id).unwrap_or_default();
        if let Some(x) = d.unsupported {
            return Err(format!("SPIR-V interface decoration {x} is not reflected"));
        }
        let input = storage == STORAGE_INPUT;
        match (d.location, d.builtin) {
            (Some(location), None) => {
                if d.flat as u8 + d.no_perspective as u8 > 1 || d.centroid && d.sample {
                    return Err("SPIR-V conflicting interpolation decorations".into());
                }
                let interpolation = if d.flat {
                    Interpolation::Flat
                } else if d.no_perspective {
                    Interpolation::NoPerspective
                } else {
                    Interpolation::Smooth
                };
                let sampling = if d.centroid {
                    Sampling::Centroid
                } else if d.sample {
                    Sampling::Sample
                } else {
                    Sampling::Center
                };
                let map = if input { &mut interface.inputs } else { &mut interface.outputs };
                if map.insert(location, Varying { component, interpolation, sampling }).is_some() {
                    return Err(format!("SPIR-V duplicate {} location {location}", if input { "input" } else { "output" }));
                }
            }
            (None, Some(builtin)) => {
                let map = if input { &mut interface.builtin_inputs } else { &mut interface.builtin_outputs };
                if map.insert(builtin, component).is_some() {
                    return Err(format!("SPIR-V duplicate builtin {builtin}"));
                }
            }
            _ => return Err("SPIR-V interface variable needs exactly one Location or BuiltIn".into()),
        }
    }
    Ok(Reflected { model, interface })
}

/// SPIR-V BuiltIn of a D3D system input of a pixel stage, with its carried component type.
pub(crate) fn fragment_builtin(system: &str) -> Option<(u32, Component)> {
    let f4 = Component { kind: Kind::Float, width: 32, lanes: 4 };
    let u1 = Component { kind: Kind::Uint, width: 32, lanes: 1 };
    Some(match system {
        "frag_coord" => (15, f4),
        "front_facing" => (17, Component { kind: Kind::Bool, width: 32, lanes: 1 }),
        "sample_index" => (18, u1),
        "primitive_index" => (7, u1),
        _ => return None,
    })
}
pub(crate) const BUILTIN_POSITION: u32 = 0;
