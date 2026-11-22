//! Tolerance-checked precise `sincos` / `rsq` / `div` receipt -> Primase emission ->
//! stock import join. Checks the controlled intrinsic on both sides and re-checks, in the actual
//! imported module, the complete WARP-exact integer edge-selection graph around it. CPU only.
use crate::compare_carrier865::stock_u32;
use crate::frac_carrier865d::uses;
use iridia::precision::{reference, ToleranceOp, TOL_EXPONENT_MASK, TOL_INVALID_NAN, TOL_MANTISSA_MASK, TOL_MEASURED_DIVISOR, TOL_QUIET_BIT, TOL_SIGN_MASK};
use iridia_naga as ir;
use naga::{Expression as E, Handle};

#[derive(Clone, Debug)]
pub struct JoinedTolerance865 {
    pub source_word: usize,
    pub op: ToleranceOp,
    pub source: ir::correspondence::ExpressionRef,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub kind: u32,
    pub operands: Vec<(u32, naga::correspondence::ExpressionRef)>,
    pub original_lanes: Vec<u8>,
}

fn kind(op: ToleranceOp) -> u32 {
    match op {
        ToleranceOp::Sin => 13,
        ToleranceOp::Cos => 14,
        ToleranceOp::Rsq => 32,
        ToleranceOp::Div => naga::correspondence::TOLERANCE_KIND_FDIV865,
    }
}

/// Stock-module twin of `iridia::precision::tolerance_graph` (same shape, stock constants).
pub(crate) fn stock_tolerance_graph(m: &naga::Module, f: &naga::Function, op: ToleranceOp, r: Handle<E>) -> bool {
    let lit = |h| stock_u32(m, f, h).or_else(|| match f.expressions[h] {
        E::Literal(naga::Literal::F32(v)) => Some(v.to_bits()),
        _ => None,
    });
    let (x, divisor) = match (op, &f.expressions[r]) {
        (ToleranceOp::Sin, &E::Math { fun: naga::MathFunction::Sin, arg, arg1: None, .. })
        | (ToleranceOp::Cos, &E::Math { fun: naga::MathFunction::Cos, arg, arg1: None, .. })
        | (ToleranceOp::Rsq, &E::Math { fun: naga::MathFunction::InverseSqrt, arg, arg1: None, .. }) => (arg, None),
        (ToleranceOp::Div, &E::Binary { op: naga::BinaryOperator::Divide, left, right }) => (left, Some(right)),
        _ => return false,
    };
    if let Some(d) = divisor {
        let fl = |h| stock_f32_lanes(m, f, h);
        if fl(d) != Some(TOL_MEASURED_DIVISOR) {
            return false;
        }
    }
    let E::As { expr: bits, kind: naga::ScalarKind::Float, convert: None } = f.expressions[x] else { return false };
    let and = |h: Handle<E>, of: Handle<E>, mask: u32| matches!(f.expressions[h], E::Binary { op: naga::BinaryOperator::And, left, right } if left == of && lit(right) == Some(mask));
    let eq = |h: Handle<E>, of: &dyn Fn(Handle<E>) -> bool, value: u32| matches!(f.expressions[h], E::Binary { op: naga::BinaryOperator::Equal, left, right } if of(left) && lit(right) == Some(value));
    if uses(f, r) != 1 || uses(f, x) != 1 {
        return false;
    }
    let Some((rb, _)) = f.expressions.iter().find(|(_, e)| matches!(e, E::As { expr, kind: naga::ScalarKind::Uint, convert: None } if *expr == r)) else { return false };
    let Some((rflush, _)) = f.expressions.iter().find(|(_, e)| matches!(**e, E::Select { condition, accept, reject }
        if reject == rb && and(accept, rb, TOL_SIGN_MASK) && eq(condition, &|h| and(h, rb, TOL_EXPONENT_MASK), 0))) else { return false };
    if uses(f, rb) != 3 || uses(f, rflush) != 1 {
        return false;
    }
    let neg_test = |h: Handle<E>| matches!(f.expressions[h], E::Binary { op: naga::BinaryOperator::NotEqual, left, right } if and(left, bits, TOL_SIGN_MASK) && lit(right) == Some(0));
    let finite = if op == ToleranceOp::Rsq {
        match f.expressions.iter().find(|(_, e)| matches!(**e, E::Select { condition, accept, reject } if reject == rflush && neg_test(condition) && lit(accept) == Some(TOL_INVALID_NAN))) {
            Some((h, _)) => h,
            None => return false,
        }
    } else {
        rflush
    };
    let pair = |h: Handle<E>, (n, p): (u32, u32)| matches!(f.expressions[h], E::Select { condition, accept, reject } if neg_test(condition) && lit(accept) == Some(n) && lit(reject) == Some(p));
    let Some((inner, _)) = f.expressions.iter().find(|(_, e)| matches!(**e, E::Select { condition, accept, reject }
        if reject == finite && pair(accept, op.zero()) && eq(condition, &|h| and(h, bits, TOL_EXPONENT_MASK), 0))) else { return false };
    let result = f.expressions.iter().any(|(_, e)| matches!(*e, E::Select { condition, accept, reject }
        if reject == inner && eq(condition, &|h| and(h, bits, TOL_EXPONENT_MASK), TOL_EXPONENT_MASK)
            && matches!(f.expressions[accept], E::Select { condition: c, accept: q, reject: inf }
                if matches!(f.expressions[c], E::Binary { op: naga::BinaryOperator::NotEqual, left, right } if and(left, bits, TOL_MANTISSA_MASK) && lit(right) == Some(0))
                && matches!(f.expressions[q], E::Binary { op: naga::BinaryOperator::InclusiveOr, left, right } if left == bits && lit(right) == Some(TOL_QUIET_BIT))
                && pair(inf, op.infinity()))));
    result && uses(f, finite) == 1 && uses(f, inner) == 1
}

/// All-equal f32 lane bits of a stock literal / constant / compose / splat.
fn stock_f32_lanes(m: &naga::Module, f: &naga::Function, h: Handle<E>) -> Option<u32> {
    fn global(m: &naga::Module, h: Handle<E>) -> Option<u32> {
        match m.global_expressions[h] {
            E::Literal(naga::Literal::F32(v)) => Some(v.to_bits()),
            E::Splat { value, .. } => global(m, value),
            E::Compose { ref components, .. } => {
                let v = components.iter().map(|c| global(m, *c)).collect::<Option<Vec<_>>>()?;
                (!v.is_empty() && v.iter().all(|x| *x == v[0])).then(|| v[0])
            }
            E::Constant(c) => global(m, m.constants[c].init),
            _ => None,
        }
    }
    match f.expressions[h] {
        E::Literal(naga::Literal::F32(v)) => Some(v.to_bits()),
        E::Splat { value, .. } => stock_f32_lanes(m, f, value),
        E::Constant(c) => global(m, m.constants[c].init),
        E::Compose { ref components, .. } => {
            let v = components.iter().map(|c| stock_f32_lanes(m, f, *c)).collect::<Option<Vec<_>>>()?;
            (!v.is_empty() && v.iter().all(|x| *x == v[0])).then(|| v[0])
        }
        _ => None,
    }
}

pub(super) fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
) -> Result<Vec<JoinedTolerance865>, String> {
    let mut out = vec![];
    for b in receipt.bindings() {
        let Some(op) = ToleranceOp::of_role(b.role) else { continue };
        let record = receipt.records().iter().find(|r| r.word == b.source_word).ok_or("ToleranceSourceMissing")?;
        let lanes = iridia::precision::tolerance_lanes(record)?;
        if lanes.op != op
            || b.control() != Some(ir::ArithmeticControl::ToleranceF32NoRefactoringV1)
            || b.original_lanes != lanes.lanes.iter().map(|l| l.0).collect::<Vec<_>>()
        {
            return Err("ToleranceReceiptShapeMismatch".into());
        }
        let source = reference(&b.producer);
        let es: Vec<_> = emitted.tolerance.iter().filter(|e| e.producer == source).collect();
        if es.len() != 1 {
            return Err("ToleranceEmissionMissingOrAmbiguous".into());
        }
        let e = es[0];
        if e.kind != kind(op)
            || e.control != Some(ir::ArithmeticControl::ToleranceF32NoRefactoringV1)
            || e.arguments.len() != b.operands.len()
            || e.arguments.iter().zip(&b.operands).any(|(a, o)| a.0 != reference(o))
        {
            return Err("ToleranceEmissionOperandControlMismatch".into());
        }
        let rows: Vec<_> = report
            .tolerance
            .iter()
            .filter(|r| match r {
                naga::correspondence::ImportedTolerance865::Direct { result_id, .. }
                | naga::correspondence::ImportedTolerance865::Unsupported { result_id, .. } => *result_id == e.result_id,
            })
            .collect();
        if rows.len() != 1 {
            return Err("ToleranceImportMissingOrAmbiguous".into());
        }
        let naga::correspondence::ImportedTolerance865::Direct { producer, result_id, type_id, kind: k, arguments, control, .. } = rows[0] else {
            return Err("ToleranceImportAdaptedOrSpilledUnsupported".into());
        };
        let expected = naga::ArithmeticControl::ToleranceF32NoRefactoringV1;
        if *type_id != e.type_id
            || *k != e.kind
            || *control != Some(expected)
            || arguments.len() != e.arguments.len()
            || arguments.iter().zip(&e.arguments).any(|(a, x)| a.0 != x.1 || a.1.owner != producer.owner)
        {
            return Err("ToleranceIdsOwnerControlMismatch".into());
        }
        let f = crate::unary_carrier::function(imported, *producer);
        if f.expression_controls.get(&producer.expression).copied() != Some(expected) {
            return Err("ToleranceActualControlMismatch".into());
        }
        for r in std::iter::once(*producer).chain(arguments.iter().map(|a| a.1)) {
            if crate::precision_carrier::stock_type(imported, r)? != (naga::ScalarKind::Float, 4, b.width.get()) {
                return Err("ToleranceActualTypeMismatch".into());
            }
        }
        if !stock_tolerance_graph(imported, f, op, producer.expression) {
            return Err("ToleranceImportedGraphMismatch".into());
        }
        out.push(JoinedTolerance865 {
            source_word: b.source_word,
            op,
            source,
            imported: *producer,
            result_id: *result_id,
            type_id: *type_id,
            kind: *k,
            operands: arguments.clone(),
            original_lanes: b.original_lanes.clone(),
        });
    }
    for e in emitted.tolerance.iter().filter(|e| e.control.is_some()) {
        if !out.iter().any(|j| j.source == e.producer) {
            return Err("ToleranceEmittedControlWithoutReceipt".into());
        }
    }
    Ok(out)
}
