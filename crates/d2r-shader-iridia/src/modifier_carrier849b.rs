//! Precise float source-operand modifiers through the SPIR-V carrier and import.
//! Checks the actual emitted words and the actual imported IR; CPU only, no numerical admission.
use std::collections::HashMap;

const OP_EXT_INST_IMPORT: u32 = 11;
const OP_EXT_INST: u32 = 12;
const OP_VECTOR_SHUFFLE: u32 = 79;
const OP_COMPOSITE_EXTRACT: u32 = 81;
const OP_F_NEGATE: u32 = 127;
const GLSL_FABS: u32 = 4;

/// Result-id definitions of the instructions this carrier inspects.
pub struct Defs {
    glsl: Option<u32>,
    defs: HashMap<u32, (u32, Vec<u32>)>,
}
pub fn defs(words: &[u32]) -> Result<Defs, String> {
    let mut out = Defs { glsl: None, defs: HashMap::new() };
    let mut at = 5;
    while at < words.len() {
        let count = (words[at] >> 16) as usize;
        let op = words[at] & 0xffff;
        if count == 0 || at + count > words.len() {
            return Err("ModifierCarrierMalformedSpirv".into());
        }
        let w = &words[at + 1..at + count];
        match op {
            OP_EXT_INST_IMPORT => {
                let name: Vec<u8> = w[1..].iter().flat_map(|x| x.to_le_bytes()).collect();
                if name.split(|b| *b == 0).next() == Some(b"GLSL.std.450") {
                    out.glsl = Some(w[0]);
                }
            }
            OP_EXT_INST | OP_VECTOR_SHUFFLE | OP_COMPOSITE_EXTRACT | OP_F_NEGATE if w.len() >= 3 => {
                if out.defs.insert(w[1], (op, w[2..].to_vec())).is_some() {
                    return Err("ModifierCarrierDuplicateResult".into());
                }
            }
            _ => {}
        }
        at += count;
    }
    Ok(out)
}
impl Defs {
    fn op(&self, id: u32) -> Option<&(u32, Vec<u32>)> {
        self.defs.get(&id)
    }
    fn fabs(&self, id: u32) -> bool {
        matches!(self.op(id), Some((OP_EXT_INST, w)) if Some(w[0]) == self.glsl && w.len() == 3 && w[1] == GLSL_FABS)
    }
    fn fneg(&self, id: u32) -> Option<u32> {
        match self.op(id) {
            Some((OP_F_NEGATE, w)) if w.len() == 1 => Some(w[0]),
            _ => None,
        }
    }
    /// The D3D modifier code an emitted operand id carries (one shuffle peeled).
    pub fn modifier(&self, id: u32) -> u32 {
        let id = match self.op(id) {
            Some((OP_VECTOR_SHUFFLE, w)) if w.len() >= 2 && w[0] == w[1] => w[0],
            Some((OP_COMPOSITE_EXTRACT, w)) if w.len() == 2 => w[0],
            _ => id,
        };
        match self.fneg(id) {
            Some(inner) if self.fabs(inner) => 3,
            Some(inner) if self.fneg(inner).is_some() => u32::MAX,
            Some(_) => 1,
            None if self.fabs(id) => 2,
            None => 0,
        }
    }
}

/// The D3D modifier code an imported stock-IR operand carries (one swizzle peeled).
pub fn imported_modifier(f: &naga::Function, operand: naga::Handle<naga::Expression>) -> u32 {
    let neg = |h| match f.expressions[h] {
        naga::Expression::Unary { op: naga::UnaryOperator::Negate, expr } => Some(expr),
        _ => None,
    };
    let abs = |h| {
        matches!(
            f.expressions[h],
            naga::Expression::Math { fun: naga::MathFunction::Abs, arg1: None, arg2: None, arg3: None, .. }
        )
    };
    let h = match f.expressions[operand] {
        naga::Expression::Swizzle { vector, .. } => vector,
        naga::Expression::AccessIndex { base, .. } => base,
        _ => operand,
    };
    match neg(h) {
        Some(inner) if abs(inner) => 3,
        Some(inner) if neg(inner).is_some() => u32::MAX,
        Some(_) => 1,
        None if abs(h) => 2,
        None => 0,
    }
}
