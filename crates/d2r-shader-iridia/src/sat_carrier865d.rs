//! Precise `add_sat` (carrier rule R4) receipt -> Primase OpFAdd (strict, mask 0)
//! -> stock import join, re-checking the exact u32 saturate graph in the imported module. CPU only.
use crate::compare_carrier865::stock_u32;
use crate::frac_carrier865d::uses;
use iridia::precision::{reference, ProducerRole, SAT_INFINITY, SAT_MEASURED_ADDENDS, SAT_ONE, SAT_SMALLEST_NORMAL};
use iridia_naga as ir;
use naga::{Expression as E, Handle};

#[derive(Clone, Debug)]
pub struct JoinedAddSat865 {
    pub source_word: usize,
    pub source: ir::correspondence::ExpressionRef,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub operands: [(u32, naga::correspondence::ExpressionRef); 2],
}

fn f32_lanes(m: &naga::Module, f: &naga::Function, h: Handle<E>) -> Option<u32> {
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
        E::Splat { value, .. } => f32_lanes(m, f, value),
        E::Constant(c) => global(m, m.constants[c].init),
        E::Compose { ref components, .. } => {
            let v = components.iter().map(|c| f32_lanes(m, f, *c)).collect::<Option<Vec<_>>>()?;
            (!v.is_empty() && v.iter().all(|x| *x == v[0])).then(|| v[0])
        }
        _ => None,
    }
}

pub(crate) fn stock_add_sat_graph(m: &naga::Module, f: &naga::Function, s: Handle<E>) -> bool {
    let E::Binary { op: naga::BinaryOperator::Add, right: c, .. } = f.expressions[s] else { return false };
    if !f32_lanes(m, f, c).is_some_and(|v| SAT_MEASURED_ADDENDS.contains(&v)) || uses(f, s) != 1 {
        return false;
    }
    let Some((rb, _)) = f.expressions.iter().find(|(_, e)| matches!(e, E::As { expr, kind: naga::ScalarKind::Uint, convert: None } if *expr == s)) else { return false };
    let lit = |h| stock_u32(m, f, h);
    let cmp = |h: Handle<E>, op: naga::BinaryOperator, v: u32| matches!(f.expressions[h], E::Binary { op: o, left, right } if o == op && left == rb && lit(right) == Some(v));
    let Some((r3, _)) = f.expressions.iter().find(|(_, e)| matches!(**e, E::Select { condition, accept, reject }
        if cmp(condition, naga::BinaryOperator::LessEqual, SAT_INFINITY) && lit(accept) == Some(SAT_ONE) && lit(reject) == Some(0))) else { return false };
    let Some((r2, _)) = f.expressions.iter().find(|(_, e)| matches!(**e, E::Select { condition, accept, reject }
        if cmp(condition, naga::BinaryOperator::LessEqual, SAT_ONE) && accept == rb && reject == r3)) else { return false };
    let result = f.expressions.iter().any(|(_, e)| matches!(*e, E::Select { condition, accept, reject }
        if cmp(condition, naga::BinaryOperator::Less, SAT_SMALLEST_NORMAL) && lit(accept) == Some(0) && reject == r2));
    result && uses(f, rb) == 4 && uses(f, r3) == 1 && uses(f, r2) == 1
}

pub(super) fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
) -> Result<Vec<JoinedAddSat865>, String> {
    let mut out = vec![];
    for b in receipt.bindings().iter().filter(|b| b.role == ProducerRole::SaturateAdd) {
        let record = receipt.records().iter().find(|r| r.word == b.source_word).ok_or("AddSatSourceMissing")?;
        let lanes = iridia::precision::add_sat_lanes(record)?;
        if b.control() != Some(ir::ArithmeticControl::AddF32StrictV1) || b.original_lanes != lanes.iter().map(|l| l.0).collect::<Vec<_>>() {
            return Err("AddSatReceiptShapeMismatch".into());
        }
        let ir::correspondence::ArithmeticEmission::Direct(e) = emitted.producer(reference(&b.producer)) else {
            return Err("AddSatEmissionNotDirect".into());
        };
        if e.opcode != 129 || (0..2).any(|k| e.arguments[k].0 != reference(&b.operands[k])) {
            return Err("AddSatEmissionOperandMismatch".into());
        }
        let rows: Vec<_> = report.arithmetic.iter().filter(|r| match r {
            naga::correspondence::ImportedArithmetic::Direct { result_id, .. } | naga::correspondence::ImportedArithmetic::Unsupported { result_id, .. } => *result_id == e.result_id,
        }).collect();
        let [naga::correspondence::ImportedArithmetic::Direct { producer, result_id, type_id, arguments, .. }] = rows[..] else {
            return Err("AddSatImportMissingOrAdapted".into());
        };
        if *type_id != e.type_id || (0..2).any(|k| arguments[k].0 != e.arguments[k].1 || arguments[k].1.owner != producer.owner) {
            return Err("AddSatIdsOwnerMismatch".into());
        }
        let f = crate::unary_carrier::function(imported, *producer);
        if f.expression_controls.get(&producer.expression).copied() != Some(naga::ArithmeticControl::AddF32StrictV1) {
            return Err("AddSatActualControlMismatch".into());
        }
        for r in [*producer, arguments[0].1, arguments[1].1] {
            if crate::precision_carrier::stock_type(imported, r)? != (naga::ScalarKind::Float, 4, b.width.get()) {
                return Err("AddSatActualTypeMismatch".into());
            }
        }
        if !stock_add_sat_graph(imported, f, producer.expression) {
            return Err("AddSatImportedGraphMismatch".into());
        }
        out.push(JoinedAddSat865 { source_word: b.source_word, source: reference(&b.producer), imported: *producer, result_id: *result_id, operands: *arguments });
    }
    Ok(out)
}
