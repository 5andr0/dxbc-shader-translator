//! Declaration-only workaround for Naga 30.0.1 SPIR-V image-coordinate lookup.
//! Missing integer coordinate vectors are declared without rewriting any source
//! instruction. This does not admit resources or extend runtime profile support.
use crate::{
    ShaderError,
    lower::{Instruction, parse},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const REVISION: &str = "image-coordinate-integer-declarations-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Declaration {
    pub result_id: u32,
    pub scalar_type_id: u32,
    pub lanes: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub revision: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub original_bound: u32,
    pub output_bound: u32,
    pub declarations: Vec<Declaration>,
    pub original_instruction_words_unchanged: bool,
}

fn unsupported(s: &str) -> ShaderError {
    ShaderError::Unsupported(s.into())
}

// These core instructions have Result Type, Result ID as their first operands.
// Unrecognized producers remain unsupported, rather than inferring operand roles.
fn typed_result(op: u16) -> bool {
    matches!(op, 41..=46 | 48..=52 | 55 | 57 | 59 | 61 | 65..=70 |
        77..=84 | 86..=98 | 100..=107 | 109..=124 | 126..=152 |
        154..=191 | 194..=205 | 207..=215 | 227 | 229..=242 | 245)
}

pub fn ensure_integer_coordinate_types(bytes: &[u8]) -> Result<(Vec<u8>, Report), ShaderError> {
    let (mut header, ins) = parse(bytes)?;
    let old_bound = header[3];
    let mut integers = BTreeSet::new();
    let mut vectors = BTreeMap::new();
    let mut images = BTreeMap::new();
    let mut values = BTreeMap::new();
    let named: BTreeSet<_> = ins
        .iter()
        .filter(|i| i.op == 5)
        .map(|i| i.args[0])
        .collect();
    for i in &ins {
        match i.op {
            21 if i.args.len() == 3 && i.args[1] == 32 && i.args[2] <= 1 => {
                integers.insert(i.args[0]);
            }
            23 if i.args.len() == 3 => {
                vectors.insert(i.args[0], (i.args[1], i.args[2]));
            }
            25 if i.args.len() >= 8 => {
                images.insert(i.args[0], i.args[2]);
            }
            _ => {}
        }
        if typed_result(i.op) && i.args.len() >= 2 {
            values.insert(i.args[1], i.args[0]);
        }
    }
    let mut needed = BTreeSet::new();
    for i in &ins {
        let (image, coordinate) = match i.op {
            95 | 98 if i.args.len() >= 4 => (i.args[2], i.args[3]),
            99 if i.args.len() >= 3 => (i.args[0], i.args[1]),
            95 | 98 | 99 => {
                return Err(ShaderError::Invalid(
                    "image coordinate instruction extent".into(),
                ));
            }
            _ => continue,
        };
        let image_ty = values
            .get(&image)
            .ok_or_else(|| unsupported("image coordinate image producer"))?;
        let dim = images
            .get(image_ty)
            .ok_or_else(|| unsupported("image coordinate image type"))?;
        let lanes = match dim {
            0 | 5 => continue,
            1 => 2,
            2 | 3 => 3,
            _ => continue,
        };
        let coordinate_ty = values
            .get(&coordinate)
            .ok_or_else(|| unsupported("image coordinate value producer"))?;
        let Some(&(scalar, _)) = vectors.get(coordinate_ty) else {
            continue;
        };
        if integers.contains(&scalar)
            && !vectors
                .iter()
                .any(|(id, v)| !named.contains(id) && *v == (scalar, lanes))
        {
            if vectors.values().any(|v| *v == (scalar, lanes)) {
                return Err(unsupported(
                    "named integer coordinate type requires frontend fix",
                ));
            }
            needed.insert((scalar, lanes));
        }
    }
    let mut declarations = Vec::new();
    for (scalar, lanes) in needed {
        let result_id = header[3];
        if result_id >= 1_048_575 {
            return Err(unsupported("image coordinate ID bound"));
        }
        header[3] += 1;
        declarations.push(Declaration {
            result_id,
            scalar_type_id: scalar,
            lanes,
        });
    }
    let output = if declarations.is_empty() {
        bytes.to_vec()
    } else {
        let first_function = ins
            .iter()
            .position(|i| i.op == 54)
            .ok_or_else(|| unsupported("image coordinate function absent"))?;
        let mut words = header.clone();
        for (at, i) in ins.iter().enumerate() {
            if at == first_function {
                for d in &declarations {
                    Instruction::new(23, vec![d.result_id, d.scalar_type_id, d.lanes])
                        .emit(&mut words);
                }
            }
            i.emit(&mut words);
        }
        words.into_iter().flat_map(u32::to_le_bytes).collect()
    };
    let report = Report {
        revision: REVISION.into(),
        input_sha256: crate::sha256(bytes),
        output_sha256: crate::sha256(&output),
        original_bound: old_bound,
        output_bound: header[3],
        declarations,
        original_instruction_words_unchanged: true,
    };
    Ok((output, report))
}

