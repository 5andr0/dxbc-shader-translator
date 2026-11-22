//! Precise source `lt` receipt -> Primase OpFOrdLessThan emission -> stock import.
//! CPU only. Grants no DXC, Vulkan, driver or renderer admission.
//!
//! The joined comparison must be the controlled ordered f32 `<` on both sides, and in the actual
//! imported module both operands must still be the exact integer denormal flush and the result
//! must still be consumed only as the D3D 0xFFFFFFFF/0 mask.
use iridia::precision::{ProducerRole, reference, LESS_EXPONENT_MASK, LESS_SIGN_MASK};
use iridia_naga as ir;

/// SPIR-V OpFOrdLessThan.
pub const OP_FORD_LESS_THAN: u32 = 184;

#[derive(Clone, Debug)]
pub struct JoinedLess865 {
    pub source_word: usize,
    pub source: ir::correspondence::ExpressionRef,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub opcode: u32,
    pub operands: [(u32, naga::correspondence::ExpressionRef); 2],
    pub original_lanes: Vec<u8>,
    pub control: naga::ArithmeticControl,
}

/// u32 lanes of a stock literal/constant/splat expression (all lanes equal), else None.
pub(crate) fn stock_u32(m: &naga::Module, f: &naga::Function, h: naga::Handle<naga::Expression>) -> Option<u32> {
    fn global(m: &naga::Module, h: naga::Handle<naga::Expression>) -> Option<u32> {
        match m.global_expressions[h] {
            naga::Expression::Literal(naga::Literal::U32(v)) => Some(v),
            naga::Expression::Splat { value, .. } => global(m, value),
            naga::Expression::Compose { ref components, .. } => {
                let v = components.iter().map(|c| global(m, *c)).collect::<Option<Vec<_>>>()?;
                (!v.is_empty() && v.iter().all(|x| *x == v[0])).then(|| v[0])
            }
            naga::Expression::Constant(c) => global(m, m.constants[c].init),
            _ => None,
        }
    }
    match f.expressions[h] {
        naga::Expression::Literal(naga::Literal::U32(v)) => Some(v),
        naga::Expression::Splat { value, .. } => stock_u32(m, f, value),
        naga::Expression::Constant(c) => global(m, m.constants[c].init),
        naga::Expression::Compose { ref components, .. } => {
            let v = components.iter().map(|c| stock_u32(m, f, *c)).collect::<Option<Vec<_>>>()?;
            (!v.is_empty() && v.iter().all(|x| *x == v[0])).then(|| v[0])
        }
        _ => None,
    }
}

/// The imported operand is exactly the integer denormal flush of some f32 value.
fn stock_flushed(m: &naga::Module, f: &naga::Function, h: naga::Handle<naga::Expression>) -> bool {
    use naga::Expression as E;
    let E::As { expr: select, kind: naga::ScalarKind::Float, convert: None } = f.expressions[h] else {
        return false;
    };
    let E::Select { condition, accept, reject: bits } = f.expressions[select] else { return false };
    // The flushed bits are the raw u32 register read.
    let E::Binary { op: naga::BinaryOperator::Equal, left: exponent, right: zero } = f.expressions[condition]
    else {
        return false;
    };
    let E::Binary { op: naga::BinaryOperator::And, left: b1, right: em } = f.expressions[exponent] else {
        return false;
    };
    let E::Binary { op: naga::BinaryOperator::And, left: b2, right: sm } = f.expressions[accept] else {
        return false;
    };
    b1 == bits
        && b2 == bits
        && stock_u32(m, f, em) == Some(LESS_EXPONENT_MASK)
        && stock_u32(m, f, sm) == Some(LESS_SIGN_MASK)
        && stock_u32(m, f, zero) == Some(0)
}

/// Exactly one consumer of the imported comparison, and it is the D3D mask select.
fn stock_mask_only(m: &naga::Module, f: &naga::Function, producer: naga::Handle<naga::Expression>) -> bool {
    use naga::Expression as E;
    let mut good = 0;
    for (_, e) in f.expressions.iter() {
        match *e {
            E::Select { condition, accept, reject } if condition == producer => {
                if stock_u32(m, f, accept) != Some(u32::MAX) || stock_u32(m, f, reject) != Some(0) {
                    return false;
                }
                good += 1;
            }
            E::Select { accept, reject, .. } if accept == producer || reject == producer => return false,
            E::Binary { left, right, .. } if left == producer || right == producer => return false,
            E::Unary { expr, .. }
            | E::As { expr, .. }
            | E::Relational { argument: expr, .. }
            | E::Swizzle { vector: expr, .. }
            | E::AccessIndex { base: expr, .. }
            | E::Splat { value: expr, .. }
                if expr == producer =>
            {
                return false
            }
            E::Compose { ref components, .. } if components.contains(&producer) => return false,
            _ => {}
        }
    }
    good == 1
}

/// Join every receipt `CompareLess` binding to exactly one emitted and one direct imported
/// OpFOrdLessThan with the controlled shape intact.
pub(super) fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
) -> Result<Vec<JoinedLess865>, String> {
    let mut out = vec![];
    // The controlled compare of a precise max is the same control and join.
    for b in receipt.bindings().iter().filter(|b| matches!(b.role, ProducerRole::CompareLess | ProducerRole::MaxCompare)) {
        let source_record = receipt
            .records()
            .iter()
            .find(|r| r.word == b.source_word)
            .ok_or("LessSourceMissing")?;
        let lanes = if b.role == ProducerRole::MaxCompare {
            iridia::precision::max_lanes(source_record)?
        } else {
            iridia::precision::less_lanes(source_record)?
        };
        let expected = b.control().ok_or("LessSourceControlMissing")?;
        if expected != ir::ArithmeticControl::LessF32NoRefactoringV1
            || !b.controlled
            || b.operands.len() != 2
            || b.original_lanes != lanes.iter().map(|l| l.0).collect::<Vec<_>>()
        {
            return Err("LessReceiptShapeMismatch".into());
        }
        let source = reference(&b.producer);
        let es = emitted.compare.iter().filter(|e| e.producer == source).collect::<Vec<_>>();
        if es.len() != 1 {
            return Err("LessEmissionMissingOrAmbiguous".into());
        }
        let e = es[0];
        if e.opcode != OP_FORD_LESS_THAN
            || e.control != Some(expected)
            || e.arguments[0].0 != reference(&b.operands[0])
            || e.arguments[1].0 != reference(&b.operands[1])
        {
            return Err("LessEmissionOperandControlMismatch".into());
        }
        let rows = report
            .compare
            .iter()
            .filter(|r| match r {
                naga::correspondence::ImportedCompare865::Direct { result_id, .. }
                | naga::correspondence::ImportedCompare865::Unsupported { result_id, .. } => {
                    *result_id == e.result_id
                }
            })
            .collect::<Vec<_>>();
        if rows.len() != 1 {
            return Err("LessImportMissingOrAmbiguous".into());
        }
        let naga::correspondence::ImportedCompare865::Direct {
            producer,
            result_id,
            type_id,
            opcode,
            arguments,
            control,
            ..
        } = rows[0]
        else {
            return Err("LessImportAdaptedOrSpilledUnsupported".into());
        };
        let control_expected = naga::ArithmeticControl::LessF32NoRefactoringV1;
        if *type_id != e.type_id
            || *opcode != OP_FORD_LESS_THAN
            || arguments[0].0 != e.arguments[0].1
            || arguments[1].0 != e.arguments[1].1
            || arguments.iter().any(|a| a.1.owner != producer.owner)
            || *control != Some(control_expected)
        {
            return Err("LessIdsOwnerControlMismatch".into());
        }
        let f = crate::unary_carrier::function(imported, *producer);
        if !matches!(f.expressions[producer.expression], naga::Expression::Binary { op: naga::BinaryOperator::Less, left, right } if left == arguments[0].1.expression && right == arguments[1].1.expression)
            || f.expression_controls.get(&producer.expression).copied() != Some(control_expected)
        {
            return Err("LessActualOperationControlMismatch".into());
        }
        for a in arguments {
            if crate::precision_carrier::stock_type(imported, a.1)?
                != (naga::ScalarKind::Float, 4, b.width.get())
            {
                return Err("LessActualTypeMismatch".into());
            }
            if b.role == ProducerRole::CompareLess && !stock_flushed(imported, f, a.1.expression) {
                return Err("LessImportedDenormalFlushMismatch".into());
            }
        }
        if crate::precision_carrier::stock_type(imported, *producer)?
            != (naga::ScalarKind::Bool, naga::BOOL_WIDTH, b.width.get())
        {
            return Err("LessActualTypeMismatch".into());
        }
        if b.role == ProducerRole::MaxCompare {
            if !crate::max_carrier865e::stock_max_graph(imported, f, producer.expression) {
                return Err("MaxImportedGraphMismatch".into());
            }
        } else if !stock_mask_only(imported, f, producer.expression) {
            return Err("LessImportedMaskMismatch".into());
        }
        out.push(JoinedLess865 {
            source_word: b.source_word,
            source,
            imported: *producer,
            result_id: *result_id,
            type_id: *type_id,
            opcode: *opcode,
            operands: *arguments,
            original_lanes: b.original_lanes.clone(),
            control: control_expected,
        });
    }
    // Every emitted controlled comparison must have come from a receipt binding.
    for e in emitted.compare.iter().filter(|e| e.control.is_some()) {
        if !out.iter().any(|j| j.source == e.producer) {
            return Err("LessEmittedControlWithoutReceipt".into());
        }
    }
    Ok(out)
}
