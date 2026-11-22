//! Precise source `frc` subtract -> Primase OpFSub -> stock import join.
//! The controlled Floor is joined by `round_ni_carrier849::join` (role `FracFloor`). This join
//! checks the NoContraction OpFSub and re-checks, in the actual imported module, the complete
//! WARP-measured graph: integer flush of the input, `x - floor(x)` on the flushed value, the
//! 1.0 -> 0x3f7fffff clamp, and the integer NaN / infinity result selection.
//! CPU only. Grants no DXC, Vulkan, driver or renderer admission.
use crate::compare_carrier865::stock_u32;
use iridia::precision::{
    reference, ProducerRole, FRAC_BELOW_ONE, FRAC_EXPONENT_MASK, FRAC_INFINITY_NAN, FRAC_MANTISSA_MASK, FRAC_ONE,
    FRAC_QUIET_BIT, FRAC_SIGN_MASK,
};
use iridia_naga as ir;
use naga::{Expression as E, Handle};

/// SPIR-V OpFSub.
pub const OP_FSUB: u32 = 131;

#[derive(Clone, Debug)]
pub struct JoinedFrac865 {
    pub source_word: usize,
    pub source: ir::correspondence::ExpressionRef,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub operands: [(u32, naga::correspondence::ExpressionRef); 2],
    pub original_lanes: Vec<u8>,
}

pub(crate) fn uses(f: &naga::Function, h: Handle<E>) -> usize {
    let mut n = 0;
    for (_, e) in f.expressions.iter() {
        n += match *e {
            E::Select { condition, accept, reject } => [condition, accept, reject].iter().filter(|x| **x == h).count(),
            E::Binary { left, right, .. } => [left, right].iter().filter(|x| **x == h).count(),
            E::Unary { expr, .. }
            | E::As { expr, .. }
            | E::Relational { argument: expr, .. }
            | E::Swizzle { vector: expr, .. }
            | E::AccessIndex { base: expr, .. }
            | E::Splat { value: expr, .. }
            | E::Load { pointer: expr } => usize::from(expr == h),
            E::Access { base, index } => [base, index].iter().filter(|x| **x == h).count(),
            E::Compose { ref components, .. } => components.iter().filter(|x| **x == h).count(),
            E::Math { arg, arg1, arg2, arg3, .. } => {
                usize::from(arg == h) + [arg1, arg2, arg3].iter().filter(|x| **x == Some(h)).count()
            }
            _ => 0,
        };
    }
    fn block(b: &naga::Block, h: Handle<E>) -> usize {
        b.iter()
            .map(|s| match s {
                naga::Statement::Store { pointer, value } => usize::from(*pointer == h) + usize::from(*value == h),
                naga::Statement::Return { value } => usize::from(*value == Some(h)),
                naga::Statement::If { condition, accept, reject } => {
                    usize::from(*condition == h) + block(accept, h) + block(reject, h)
                }
                naga::Statement::Block(b) => block(b, h),
                _ => 0,
            })
            .sum()
    }
    n + block(&f.body, h)
}

/// The imported graph around the subtract `d`; true when every node is exactly as admitted.
pub(crate) fn stock_frac_graph(m: &naga::Module, f: &naga::Function, d: Handle<E>) -> bool {
    let lit = |h| stock_u32(m, f, h);
    let E::Binary { op: naga::BinaryOperator::Subtract, left: x, right: fl } = f.expressions[d] else { return false };
    if !matches!(f.expressions[fl], E::Math { fun: naga::MathFunction::Floor, arg, arg1: None, arg2: None, arg3: None } if arg == x) {
        return false;
    }
    let E::As { expr: select, kind: naga::ScalarKind::Float, convert: None } = f.expressions[x] else { return false };
    let E::Select { condition: zero_test, accept: signed_zero, reject: bits } = f.expressions[select] else { return false };
    // The flushed bits are the raw u32 register read.
    let E::Binary { op: naga::BinaryOperator::Equal, left: exponent, right: zero } = f.expressions[zero_test] else { return false };
    let and = |h, mask| matches!(f.expressions[h], E::Binary { op: naga::BinaryOperator::And, left, right } if left == bits && lit(right) == Some(mask));
    if !and(exponent, FRAC_EXPONENT_MASK) || !and(signed_zero, FRAC_SIGN_MASK) || lit(zero) != Some(0) {
        return false;
    }
    if uses(f, d) != 1 || uses(f, fl) != 1 {
        return false;
    }
    let Some((db, _)) = f.expressions.iter().find(|(_, e)| matches!(e, E::As { expr, kind: naga::ScalarKind::Uint, convert: None } if *expr == d)) else {
        return false;
    };
    // The NSZ sign mask.
    let Some((db_abs, _)) = f.expressions.iter().find(|(_, e)| matches!(**e, E::Binary { op: naga::BinaryOperator::And, left, right }
        if left == db && lit(right) == Some(0x7fff_ffff))) else {
        return false;
    };
    if uses(f, db) != 1 {
        return false;
    }
    let db = db_abs;
    let Some((clamped, (test, below))) = f.expressions.iter().find_map(|(h, e)| match *e {
        E::Select { condition, accept, reject } if reject == db => Some((h, (condition, accept))),
        _ => None,
    }) else {
        return false;
    };
    let one = matches!(f.expressions[test], E::Binary { op: naga::BinaryOperator::Equal, left, right } if left == db && lit(right) == Some(FRAC_ONE));
    if !one || lit(below) != Some(FRAC_BELOW_ONE) || uses(f, db) != 2 || uses(f, test) != 1 || uses(f, clamped) != 1 {
        return false;
    }
    let Some((special, nan)) = f.expressions.iter().find_map(|(_, e)| match *e {
        E::Select { condition, accept, reject } if reject == clamped => Some((condition, accept)),
        _ => None,
    }) else {
        return false;
    };
    let special_ok = matches!(f.expressions[special], E::Binary { op: naga::BinaryOperator::Equal, left, right } if left == exponent && lit(right) == Some(FRAC_EXPONENT_MASK));
    let E::Select { condition: is_nan, accept: quiet, reject: infinity } = f.expressions[nan] else { return false };
    special_ok
        && matches!(f.expressions[is_nan], E::Binary { op: naga::BinaryOperator::NotEqual, left, right } if lit(right) == Some(0) && and(left, FRAC_MANTISSA_MASK))
        && matches!(f.expressions[quiet], E::Binary { op: naga::BinaryOperator::InclusiveOr, left, right } if left == bits && lit(right) == Some(FRAC_QUIET_BIT))
        && lit(infinity) == Some(FRAC_INFINITY_NAN)
}

pub(super) fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
    floors: &[crate::round_ni_carrier849::JoinedRoundNi],
) -> Result<Vec<JoinedFrac865>, String> {
    let mut out = vec![];
    for b in receipt.bindings().iter().filter(|b| b.role == ProducerRole::FracSubtract) {
        let record = receipt.records().iter().find(|r| r.word == b.source_word).ok_or("FracSourceMissing")?;
        let lanes = iridia::precision::frac_lanes(record)?;
        if b.control() != Some(ir::ArithmeticControl::NoContraction)
            || b.operands.len() != 2
            || b.original_lanes != lanes.iter().map(|l| l.0).collect::<Vec<_>>()
        {
            return Err("FracReceiptShapeMismatch".into());
        }
        let ir::correspondence::ArithmeticEmission::Direct(e) = emitted.producer(reference(&b.producer)) else {
            return Err("FracEmissionNotDirect".into());
        };
        if e.opcode != OP_FSUB || (0..2).any(|k| e.arguments[k].0 != reference(&b.operands[k])) {
            return Err("FracEmissionOperandMismatch".into());
        }
        let rows: Vec<_> = report
            .arithmetic
            .iter()
            .filter(|r| match r {
                naga::correspondence::ImportedArithmetic::Direct { result_id, .. }
                | naga::correspondence::ImportedArithmetic::Unsupported { result_id, .. } => *result_id == e.result_id,
            })
            .collect();
        if rows.len() != 1 {
            return Err("FracImportMissingOrAmbiguous".into());
        }
        let naga::correspondence::ImportedArithmetic::Direct { producer, result_id, type_id, arguments, .. } = rows[0] else {
            return Err("FracImportAdaptedOrSpilledUnsupported".into());
        };
        if *type_id != e.type_id
            || (0..2).any(|k| arguments[k].0 != e.arguments[k].1 || arguments[k].1.owner != producer.owner)
        {
            return Err("FracIdsOwnerMismatch".into());
        }
        let f = crate::unary_carrier::function(imported, *producer);
        if !matches!(f.expressions[producer.expression], E::Binary { op: naga::BinaryOperator::Subtract, left, right }
                if left == arguments[0].1.expression && right == arguments[1].1.expression)
            || f.expression_controls.get(&producer.expression).copied() != Some(naga::ArithmeticControl::NoContraction)
        {
            return Err("FracActualOperationControlMismatch".into());
        }
        // The subtract's right operand is the joined controlled Floor of the same source word.
        let floor = floors
            .iter()
            .filter(|j| j.source_word == b.source_word)
            .collect::<Vec<_>>();
        if floor.len() != 1 || floor[0].imported != arguments[1].1 || floor[0].operand.1 != arguments[0].1 {
            return Err("FracFloorJoinMismatch".into());
        }
        for r in [*producer, arguments[0].1, arguments[1].1] {
            if crate::precision_carrier::stock_type(imported, r)? != (naga::ScalarKind::Float, 4, b.width.get()) {
                return Err("FracActualTypeMismatch".into());
            }
        }
        if !stock_frac_graph(imported, f, producer.expression) {
            return Err("FracImportedGraphMismatch".into());
        }
        out.push(JoinedFrac865 {
            source_word: b.source_word,
            source: reference(&b.producer),
            imported: *producer,
            result_id: *result_id,
            type_id: *type_id,
            operands: *arguments,
            original_lanes: b.original_lanes.clone(),
        });
    }
    Ok(out)
}
