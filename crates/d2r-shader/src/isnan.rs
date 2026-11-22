//! Exact IEEE-754 f32 NaN classification for WGSL's missing IsNan operation.
//! Only the predicate representation changes; float values/math are untouched.
use crate::{
    ShaderError,
    lower::{Instruction, parse},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const REVISION: &str = "isnan-f32-ieee-bit-classification-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rewrite {
    pub instruction_index: usize,
    pub lanes: u32,
    pub original: Vec<u32>,
    pub replacement: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub revision: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub original_bound: u32,
    pub output_bound: u32,
    pub declarations: Vec<u32>,
    pub rewrites: Vec<Rewrite>,
    pub original_float_math_and_control_flow_unchanged: bool,
}
fn bad(s: &str) -> ShaderError {
    ShaderError::Unsupported(s.into())
}
fn typed_result(op: u16) -> bool {
    matches!(op, 1 | 12 | 41..=46 | 48..=52 | 55 | 57 | 59 | 61 | 65..=70 |
        77..=84 | 86..=98 | 100..=107 | 109..=124 | 126..=152 |
        154..=191 | 194..=205 | 207..=215 | 227 | 229..=242 | 245)
}
struct Builder {
    next: u32,
    declarations: Vec<u32>,
}
impl Builder {
    fn id(&mut self) -> Result<u32, ShaderError> {
        if self.next >= 1_048_575 {
            return Err(bad("IsNan ID bound"));
        }
        let id = self.next;
        self.next += 1;
        Ok(id)
    }
    fn constant(&mut self, ty: u32, bits: u32) -> Result<u32, ShaderError> {
        let id = self.id()?;
        Instruction::new(43, vec![ty, id, bits]).emit(&mut self.declarations);
        Ok(id)
    }
    fn splat(&mut self, ty: u32, scalar: u32, lanes: u32) -> Result<u32, ShaderError> {
        if lanes == 1 {
            return Ok(scalar);
        }
        let id = self.id()?;
        let mut args = vec![ty, id];
        args.extend(vec![scalar; lanes as usize]);
        Instruction::new(44, args).emit(&mut self.declarations);
        Ok(id)
    }
}

pub fn lower_f32_isnan(bytes: &[u8]) -> Result<(Vec<u8>, Report), ShaderError> {
    let (header, ins) = parse(bytes)?;
    let mut report = Report {
        revision: REVISION.into(),
        input_sha256: crate::sha256(bytes),
        output_sha256: crate::sha256(bytes),
        original_bound: header[3],
        output_bound: header[3],
        declarations: vec![],
        rewrites: vec![],
        original_float_math_and_control_flow_unchanged: true,
    };
    if !ins.iter().any(|i| i.op == 156) {
        return Ok((bytes.to_vec(), report));
    }
    let first_function = ins
        .iter()
        .position(|i| i.op == 54)
        .ok_or_else(|| bad("IsNan function absent"))?;
    let mut scalar = BTreeMap::new();
    let mut vectors = BTreeMap::new();
    let mut values = BTreeMap::new();
    for i in &ins {
        match i.op {
            20 if i.args.len() == 1 => {
                scalar.insert(i.args[0], "bool");
            }
            21 if i.args.len() == 3 && i.args[1..] == [32, 0] => {
                scalar.insert(i.args[0], "u32");
            }
            22 if i.args.len() == 2 && i.args[1] == 32 => {
                scalar.insert(i.args[0], "f32");
            }
            23 if i.args.len() == 3 && (2..=4).contains(&i.args[2]) => {
                vectors.insert(i.args[0], (i.args[1], i.args[2]));
            }
            _ => {}
        }
        if typed_result(i.op) && i.args.len() >= 2 {
            values.insert(i.args[1], i.args[0]);
        }
    }
    let shape = |id: u32| -> Result<(u32, u32), ShaderError> {
        if scalar.contains_key(&id) {
            Ok((id, 1))
        } else {
            vectors
                .get(&id)
                .copied()
                .ok_or_else(|| bad("IsNan scalar/vector32 type"))
        }
    };
    let mut builder = Builder {
        next: header[3],
        declarations: vec![],
    };
    let mut uint = scalar
        .iter()
        .find_map(|(&id, &kind)| (kind == "u32").then_some(id));
    let mut helpers = BTreeMap::new();
    let mut replacements = BTreeMap::new();
    for (at, i) in ins.iter().enumerate().filter(|(_, i)| i.op == 156) {
        if i.args.len() != 3 || at < first_function {
            return Err(bad("IsNan instruction extent or scope"));
        }
        let [bool_ty, result, operand] = <[u32; 3]>::try_from(i.args.as_slice()).unwrap();
        let operand_ty = *values
            .get(&operand)
            .ok_or_else(|| bad("IsNan unknown operand producer"))?;
        let (float_scalar, lanes) = shape(operand_ty)?;
        let (bool_scalar, bool_lanes) = shape(bool_ty)?;
        if scalar.get(&float_scalar) != Some(&"f32")
            || scalar.get(&bool_scalar) != Some(&"bool")
            || lanes != bool_lanes
        {
            return Err(bad(
                "IsNan requires matching f32/bool scalar or vector shape",
            ));
        }
        let u32_ty = if let Some(id) = uint {
            id
        } else {
            let id = builder.id()?;
            Instruction::new(21, vec![id, 32, 0]).emit(&mut builder.declarations);
            uint = Some(id);
            id
        };
        let (uint_ty, exponent, mantissa, zero, false_value) =
            if let Some(&v) = helpers.get(&(bool_ty, lanes)) {
                v
            } else {
                let uint_ty = if lanes == 1 {
                    u32_ty
                } else if let Some(id) = vectors
                    .iter()
                    .find_map(|(&id, &v)| (v == (u32_ty, lanes)).then_some(id))
                {
                    id
                } else {
                    let id = builder.id()?;
                    Instruction::new(23, vec![id, u32_ty, lanes]).emit(&mut builder.declarations);
                    id
                };
                let exp = builder.constant(u32_ty, 0x7f800000)?;
                let man = builder.constant(u32_ty, 0x007fffff)?;
                let zero = builder.constant(u32_ty, 0)?;
                let exp = builder.splat(uint_ty, exp, lanes)?;
                let man = builder.splat(uint_ty, man, lanes)?;
                let zero = builder.splat(uint_ty, zero, lanes)?;
                let f = builder.id()?;
                Instruction::new(42, vec![bool_scalar, f]).emit(&mut builder.declarations);
                let f = builder.splat(bool_ty, f, lanes)?;
                let value = (uint_ty, exp, man, zero, f);
                helpers.insert((bool_ty, lanes), value);
                value
            };
        let bits = builder.id()?;
        let exponent_bits = builder.id()?;
        let exponent_all_ones = builder.id()?;
        let mantissa_bits = builder.id()?;
        let mantissa_nonzero = builder.id()?;
        let replacement = vec![
            Instruction::new(124, vec![uint_ty, bits, operand]),
            Instruction::new(199, vec![uint_ty, exponent_bits, bits, exponent]),
            Instruction::new(
                170,
                vec![bool_ty, exponent_all_ones, exponent_bits, exponent],
            ),
            Instruction::new(199, vec![uint_ty, mantissa_bits, bits, mantissa]),
            Instruction::new(171, vec![bool_ty, mantissa_nonzero, mantissa_bits, zero]),
            Instruction::new(
                169,
                vec![
                    bool_ty,
                    result,
                    exponent_all_ones,
                    mantissa_nonzero,
                    false_value,
                ],
            ),
        ];
        let mut original_words = vec![];
        i.emit(&mut original_words);
        let mut replacement_words = vec![];
        for instruction in &replacement {
            instruction.emit(&mut replacement_words);
        }
        report.rewrites.push(Rewrite {
            instruction_index: at,
            lanes,
            original: original_words,
            replacement: replacement_words,
        });
        replacements.insert(at, replacement);
    }
    let mut words = header;
    words[3] = builder.next;
    for (at, i) in ins.iter().enumerate() {
        if at == first_function {
            words.extend_from_slice(&builder.declarations);
        }
        if let Some(replacement) = replacements.get(&at) {
            for r in replacement {
                r.emit(&mut words);
            }
        } else {
            i.emit(&mut words);
        }
    }
    let output: Vec<_> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    report.output_bound = builder.next;
    report.output_sha256 = crate::sha256(&output);
    report.declarations = builder.declarations;
    Ok((output, report))
}

