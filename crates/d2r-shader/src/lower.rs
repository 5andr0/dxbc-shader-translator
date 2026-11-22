//! Authored deterministic SPIR-V rewrite for a declared R32_UINT SRV.
//! Only x-component fetch consumers are supported. No substitute RGBA defaults.

use crate::{BufferBinding, ShaderError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct Instruction {
    pub(crate) op: u16,
    pub(crate) args: Vec<u32>,
}
impl Instruction {
    pub(crate) fn new(op: u16, args: impl Into<Vec<u32>>) -> Self {
        Self {
            op,
            args: args.into(),
        }
    }
    pub(crate) fn emit(&self, out: &mut Vec<u32>) {
        out.push((((self.args.len() + 1) as u32) << 16) | self.op as u32);
        out.extend_from_slice(&self.args);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoweringReport {
    pub bindings: Vec<LoweredBinding>,
    pub fetches_replaced: usize,
    pub extracts_replaced: usize,
    pub image_loads_removed: usize,
    pub source_only_x_consumers_proved: bool,
    pub helper_function_id: Option<u32>,
    pub helper_words: Vec<u32>,
    pub helper_zero_id: Option<u32>,
    pub helper_buffer_id: Option<u32>,
    pub unchanged_other_instructions: usize,
    pub original_instruction_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoweredBinding {
    pub source: BufferBinding,
    pub target_kind: String,
    pub element_stride_bytes: u32,
    pub exact_view_byte_length_required: bool,
    pub nonempty_binding_required: bool,
    pub out_of_bounds: String,
}

fn bad(reason: impl Into<String>) -> ShaderError {
    ShaderError::Unsupported(reason.into())
}
pub(crate) fn parse(bytes: &[u8]) -> Result<(Vec<u32>, Vec<Instruction>), ShaderError> {
    if bytes.len() < 20 || bytes.len() % 4 != 0 || bytes.len() > 4 * 1024 * 1024 {
        return Err(ShaderError::Invalid("SPIR-V byte extent".into()));
    }
    let words: Vec<u32> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if words[0] != 0x07230203 || words[4] != 0 || words[3] == 0 || words[3] > 1_048_576 {
        return Err(ShaderError::Invalid("SPIR-V header".into()));
    }
    let mut cursor = 5;
    let mut instructions = Vec::new();
    while cursor < words.len() {
        let count = (words[cursor] >> 16) as usize;
        if count == 0 || cursor + count > words.len() {
            return Err(ShaderError::Invalid("SPIR-V instruction extent".into()));
        }
        let op = words[cursor] as u16;
        let args = words[cursor + 1..cursor + count].to_vec();
        // Guard every type result ID inspected by later passes, including types
        // that this profile otherwise leaves unchanged. Malformed input must not
        // become an indexing panic before Naga can reject it.
        let minimum = match op {
            5 | 6 => 2,
            8 => 3,
            10 | 17 => 1,
            15 => 3,
            19 | 20 | 26 | 34 | 35 | 36 | 37 => 1,
            21 | 23 | 24 | 28 | 32 => 3,
            22 | 27 | 29 | 31 | 33 | 38 | 39 => 2,
            25 => 8,
            30 => 1,
            41 | 42 => 2,
            43 => 3,
            54 => 4,
            55 => 2,
            57 => 3,
            59 => 3,
            61 => 3,
            62 => 2,
            65 => 4,
            68 => 4,
            71 => 2,
            72 => 3,
            81 => 4,
            95 => 4,
            124 => 3,
            128 | 130 | 132 | 194..=199 => 4,
            200 => 3,
            _ => 0,
        };
        if args.len() < minimum {
            return Err(ShaderError::Invalid(format!("opcode{op} operand extent")));
        }
        instructions.push(Instruction::new(op, args));
        cursor += count;
    }
    Ok((words[..5].to_vec(), instructions))
}

pub fn lower_r32_uint_x_only(
    bytes: &[u8],
    contracts: &[BufferBinding],
) -> Result<(Vec<u8>, LoweringReport), ShaderError> {
    let (mut header, ins) = parse(bytes)?;
    let buffer_images: Vec<u32> = ins
        .iter()
        .filter(|i| i.op == 25 && i.args.get(2) == Some(&5)) // DimBuffer
        .map(|i| i.args[0])
        .collect();
    let mut report = LoweringReport {
        bindings: Vec::new(),
        fetches_replaced: 0,
        extracts_replaced: 0,
        image_loads_removed: 0,
        source_only_x_consumers_proved: false,
        helper_function_id: None,
        helper_words: Vec::new(),
        helper_zero_id: None,
        helper_buffer_id: None,
        unchanged_other_instructions: 0,
        original_instruction_count: ins.len(),
    };
    if buffer_images.is_empty() {
        return Ok((bytes.to_vec(), report));
    }
    if buffer_images.len() != 1 {
        return Err(bad(
            "this revision supports exactly one declared buffer image",
        ));
    }
    let image = buffer_images[0];
    let image_decl = ins
        .iter()
        .find(|i| i.op == 25 && i.args[0] == image)
        .unwrap();
    if image_decl.args.len() != 8
        || image_decl.args[3] != 2
        || image_decl.args[4] != 0
        || image_decl.args[5] != 0
        || image_decl.args[6] != 1
        || image_decl.args[7] != 0
    {
        return Err(bad("unsupported buffer image declaration"));
    }
    let uint = image_decl.args[1];
    if !ins.iter().any(|i| i.op == 21 && i.args == [uint, 32, 0]) {
        return Err(bad("R32_UINT requires unsigned 32-bit sampled type"));
    }
    let ptrs: Vec<u32> = ins
        .iter()
        .filter(|i| i.op == 32 && i.args.len() == 3 && i.args[1] == 0 && i.args[2] == image)
        .map(|i| i.args[0])
        .collect();
    if ptrs.len() != 1 {
        return Err(bad("buffer image pointer profile"));
    }
    let old_ptr = ptrs[0];
    let vars: Vec<u32> = ins
        .iter()
        .filter(|i| i.op == 59 && i.args.len() == 3 && i.args[0] == old_ptr && i.args[2] == 0)
        .map(|i| i.args[1])
        .collect();
    if vars.len() != 1 {
        return Err(bad("buffer image variable profile"));
    }
    let buffer = vars[0];
    let decoration = |kind| {
        ins.iter()
            .find(|i| i.op == 71 && i.args.len() == 3 && i.args[0] == buffer && i.args[1] == kind)
            .map(|i| i.args[2])
    };
    let group = decoration(34).ok_or_else(|| bad("buffer DescriptorSet absent"))?;
    let binding = decoration(33).ok_or_else(|| bad("buffer Binding absent"))?;
    let matches: Vec<_> = contracts
        .iter()
        .filter(|c| c.group == group && c.binding == binding && c.format == "R32_UINT")
        .collect();
    if matches.len() != 1 {
        return Err(bad("one explicit R32_UINT view contract is required"));
    }
    let loads: BTreeSet<u32> = ins
        .iter()
        .filter(|i| i.op == 61 && i.args.len() == 3 && i.args[0] == image && i.args[2] == buffer)
        .map(|i| i.args[1])
        .collect();
    if loads.is_empty() {
        return Err(bad("buffer has no recognized image load"));
    }
    let mut fetches = BTreeMap::new();
    for i in &ins {
        if i.op == 95 && i.args.len() >= 4 && loads.contains(&i.args[2]) {
            if i.args.len() != 4 {
                return Err(bad("buffer ImageFetch optional operands unsupported"));
            }
            let ty = i.args[0];
            if !ins.iter().any(|v| v.op == 23 && v.args == [ty, uint, 4]) {
                return Err(bad("fetch result must be uint4"));
            }
            fetches.insert(i.args[1], i.args[3]);
        }
    }
    if fetches.is_empty() {
        return Err(bad("buffer has no recognized fetch"));
    }
    let mut extracts = BTreeMap::new();
    for i in &ins {
        if i.op == 81 && i.args.len() >= 3 && fetches.contains_key(&i.args[2]) {
            if i.args.len() != 4 || i.args[0] != uint || i.args[3] != 0 {
                return Err(bad("only used fetch x-component is proved"));
            }
            extracts.insert(i.args[1], fetches[&i.args[2]]);
        }
    }
    for id in fetches.keys() {
        if !ins.iter().any(|i| i.op == 81 && i.args.get(2) == Some(id)) {
            return Err(bad("fetch without recognized scalar consumer"));
        }
    }
    // Only function instruction operands are examined; debug OpLine operands
    // include literal source line numbers, not value references.
    let mut in_function = false;
    for i in &ins {
        if i.op == 54 {
            in_function = true;
        }
        if in_function && i.op != 8 {
            for (at, id) in i.args.iter().enumerate() {
                if loads.contains(id) && !((i.op == 61 && at == 1) || (i.op == 95 && at == 2)) {
                    return Err(bad(format!(
                        "image handle used by opcode{} operand{}",
                        i.op, at
                    )));
                }
                if fetches.contains_key(id) && !((i.op == 95 && at == 1) || (i.op == 81 && at == 2))
                {
                    return Err(bad(format!(
                        "fetch result used by opcode{} operand{}",
                        i.op, at
                    )));
                }
            }
        }
        if i.op == 56 {
            in_function = false;
        }
    }
    let mut next = header[3];
    let mut id = || {
        let v = next;
        next += 1;
        v
    };
    let array = id();
    let structure = id();
    let ptr_struct = id();
    let ptr_elem = id();
    let bool_ty = ins
        .iter()
        .find(|i| i.op == 20)
        .map(|i| i.args[0])
        .unwrap_or_else(&mut id);
    let zero = ins
        .iter()
        .find(|i| i.op == 43 && i.args.len() == 3 && i.args[0] == uint && i.args[2] == 0)
        .map(|i| i.args[1])
        .unwrap_or_else(&mut id);
    let fn_ty = id();
    let helper = id();
    let param = id();
    let entry = id();
    let len = id();
    let valid = id();
    let inside = id();
    let outside = id();
    let merge = id();
    let element = id();
    let value = id();
    let result = id();
    let decorations = vec![
        Instruction::new(71, vec![array, 6, 4]),
        Instruction::new(71, vec![structure, 3]),
        Instruction::new(72, vec![structure, 0, 35, 0]),
        Instruction::new(72, vec![structure, 0, 24]),
    ];
    let mut declarations = vec![
        Instruction::new(29, vec![array, uint]),
        Instruction::new(30, vec![structure, array]),
        Instruction::new(32, vec![ptr_struct, 2, structure]),
        Instruction::new(32, vec![ptr_elem, 2, uint]),
    ];
    if !ins.iter().any(|i| i.op == 20) {
        declarations.push(Instruction::new(20, vec![bool_ty]));
    }
    if !ins
        .iter()
        .any(|i| i.op == 43 && i.args.len() == 3 && i.args[0] == uint && i.args[2] == 0)
    {
        declarations.push(Instruction::new(43, vec![uint, zero, 0]));
    }
    declarations.push(Instruction::new(33, vec![fn_ty, uint, uint]));
    declarations.push(Instruction::new(59, vec![ptr_struct, buffer, 2]));
    let helper_code = vec![
        Instruction::new(54, vec![uint, helper, 0, fn_ty]),
        Instruction::new(55, vec![uint, param]),
        Instruction::new(248, vec![entry]),
        Instruction::new(68, vec![uint, len, buffer, 0]),
        Instruction::new(176, vec![bool_ty, valid, param, len]),
        Instruction::new(247, vec![merge, 0]),
        Instruction::new(250, vec![valid, inside, outside]),
        Instruction::new(248, vec![inside]),
        Instruction::new(65, vec![ptr_elem, element, buffer, zero, param]),
        Instruction::new(61, vec![uint, value, element]),
        Instruction::new(249, vec![merge]),
        Instruction::new(248, vec![outside]),
        Instruction::new(249, vec![merge]),
        Instruction::new(248, vec![merge]),
        Instruction::new(245, vec![uint, result, value, inside, zero, outside]),
        Instruction::new(254, vec![result]),
        Instruction::new(56, vec![]),
    ];
    let mut output = Vec::new();
    let mut decorated = false;
    let mut functions = false;
    for i in &ins {
        if !decorated && matches!(i.op, 19..=39) {
            for d in &decorations {
                d.emit(&mut output);
            }
            decorated = true;
        }
        if !functions && i.op == 54 {
            for d in &declarations {
                d.emit(&mut output);
            }
            for d in &helper_code {
                d.emit(&mut output);
            }
            functions = true;
        }
        if (i.op == 17 && i.args == [46])
            || (i.op == 25 && i.args[0] == image)
            || (i.op == 32 && i.args[0] == old_ptr)
            || (i.op == 59 && i.args.get(1) == Some(&buffer))
        {
            continue;
        }
        if i.op == 61 && i.args.len() == 3 && loads.contains(&i.args[1]) {
            report.image_loads_removed += 1;
            continue;
        }
        if i.op == 95 && i.args.len() == 4 && fetches.contains_key(&i.args[1]) {
            report.fetches_replaced += 1;
            continue;
        }
        if i.op == 81 && i.args.len() == 4 && extracts.contains_key(&i.args[1]) {
            let coord = extracts[&i.args[1]];
            let coord_ty = ins
                .iter()
                .find(|v| {
                    v.args.len() > 1
                        && v.args[1] == coord
                        && v.op != 5
                        && v.op != 71
                        && v.op != 72
                        && v.op != 8
                })
                .map(|v| v.args[0])
                .ok_or_else(|| bad("coordinate definition missing"))?;
            if coord_ty != uint {
                return Err(bad(
                    "this revision requires the existing u32 coordinate, no implicit cast",
                ));
            }
            Instruction::new(57, vec![uint, i.args[1], helper, coord]).emit(&mut output);
            report.extracts_replaced += 1;
            continue;
        }
        i.emit(&mut output);
        report.unchanged_other_instructions += 1;
    }
    if !decorated || !functions {
        return Err(bad("module section layout"));
    }
    header[3] = next;
    header.extend(output);
    for i in &helper_code {
        i.emit(&mut report.helper_words);
    }
    report.bindings.push(LoweredBinding {
        source: matches[0].clone(),
        target_kind: "readonly-storage-u32-runtime-array".into(),
        element_stride_bytes: 4,
        exact_view_byte_length_required: true,
        nonempty_binding_required: true,
        out_of_bounds: "explicit unsigned index < exact bound arrayLength, otherwise uint0".into(),
    });
    report.source_only_x_consumers_proved = true;
    report.helper_function_id = Some(helper);
    report.helper_zero_id = Some(zero);
    report.helper_buffer_id = Some(buffer);
    Ok((
        header.into_iter().flat_map(u32::to_le_bytes).collect(),
        report,
    ))
}
