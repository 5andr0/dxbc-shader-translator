//! Precise source `round_ni` receipt -> Primase Floor emission -> stock import join.
//! CPU only. Grants no numerical (denormal/FTZ), DXC, Vulkan or renderer admission.
use iridia::precision::{ProducerRole, reference};
use iridia_naga as ir;

/// GLSL.std.450 Floor.
pub const GLSL_STD_450_FLOOR: u32 = 8;

#[derive(Clone, Debug)]
pub struct JoinedRoundNi {
    pub source_word: usize,
    pub source: ir::correspondence::ExpressionRef,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub set_id: u32,
    pub instruction: u32,
    pub operand: (u32, naga::correspondence::ExpressionRef),
    pub original_lanes: Vec<u8>,
    pub control: naga::ArithmeticControl,
}

fn stock(c: ir::ArithmeticControl) -> naga::ArithmeticControl {
    match c {
        ir::ArithmeticControl::NoContraction => naga::ArithmeticControl::NoContraction,
        ir::ArithmeticControl::Exp2F32NoRefactoringV1 => {
            naga::ArithmeticControl::Exp2F32NoRefactoringV1
        }
        ir::ArithmeticControl::FloorF32NoRefactoringV1 => {
            naga::ArithmeticControl::FloorF32NoRefactoringV1
        }
        ir::ArithmeticControl::LessF32NoRefactoringV1 => {
            naga::ArithmeticControl::LessF32NoRefactoringV1
        }
        ir::ArithmeticControl::ToleranceF32NoRefactoringV1 => {
            naga::ArithmeticControl::ToleranceF32NoRefactoringV1
        }
        ir::ArithmeticControl::AddF32StrictV1 => naga::ArithmeticControl::AddF32StrictV1,
    }
}

/// Join every receipt `RoundNi` binding to exactly one emitted and one direct imported Floor.
pub(super) fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
) -> Result<Vec<JoinedRoundNi>, String> {
    let mut out = vec![];
    // The controlled Floor of a precise frc is the same Floor control and join.
    for b in receipt.bindings().iter().filter(|b| matches!(b.role, ProducerRole::RoundNi | ProducerRole::FracFloor)) {
        let source_record = receipt
            .records()
            .iter()
            .find(|r| r.word == b.source_word)
            .ok_or("RoundNiSourceMissing")?;
        let lanes: Vec<(u8, u8)> = if b.role == ProducerRole::FracFloor {
            iridia::precision::frac_lanes(source_record)?
        } else {
            iridia::precision::round_ni_lanes(source_record)?
        };
        let expected = b.control().ok_or("RoundNiSourceControlMissing")?;
        if expected != ir::ArithmeticControl::FloorF32NoRefactoringV1
            || !b.controlled
            || b.operands.len() != 1
            || b.original_lanes != lanes.iter().map(|l| l.0).collect::<Vec<_>>()
        {
            return Err("RoundNiReceiptShapeMismatch".into());
        }
        let source = reference(&b.producer);
        let es = emitted.floor.iter().filter(|e| e.producer == source).collect::<Vec<_>>();
        if es.len() != 1 {
            return Err("RoundNiEmissionMissingOrAmbiguous".into());
        }
        let e = es[0];
        if e.instruction != GLSL_STD_450_FLOOR
            || e.control != Some(expected)
            || e.arguments[0].0 != reference(&b.operands[0])
        {
            return Err("RoundNiEmissionOperandControlMismatch".into());
        }
        let rows = report
            .floor
            .iter()
            .filter(|r| match r {
                naga::correspondence::ImportedFloor::Direct { result_id, .. }
                | naga::correspondence::ImportedFloor::Unsupported { result_id, .. } => {
                    *result_id == e.result_id
                }
            })
            .collect::<Vec<_>>();
        if rows.len() != 1 {
            return Err("RoundNiImportMissingOrAmbiguous".into());
        }
        let naga::correspondence::ImportedFloor::Direct {
            producer,
            result_id,
            type_id,
            set_id,
            instruction,
            arguments,
            control,
            ..
        } = rows[0]
        else {
            return Err("RoundNiImportAdaptedOrSpilledUnsupported".into());
        };
        let control_expected = stock(expected);
        if *type_id != e.type_id
            || *set_id != e.set_id
            || *instruction != GLSL_STD_450_FLOOR
            || arguments[0].0 != e.arguments[0].1
            || arguments[0].1.owner != producer.owner
            || *control != Some(control_expected)
        {
            return Err("RoundNiIdsOwnerControlMismatch".into());
        }
        let f = crate::unary_carrier::function(imported, *producer);
        if !matches!(f.expressions[producer.expression], naga::Expression::Math { fun: naga::MathFunction::Floor, arg, arg1: None, arg2: None, arg3: None } if arg == arguments[0].1.expression)
            || f.expression_controls.get(&producer.expression).copied() != Some(control_expected)
        {
            return Err("RoundNiActualOperationControlMismatch".into());
        }
        for r in [*producer, arguments[0].1] {
            if crate::precision_carrier::stock_type(imported, r)?
                != (naga::ScalarKind::Float, 4, b.width.get())
            {
                return Err("RoundNiActualTypeMismatch".into());
            }
        }
        out.push(JoinedRoundNi {
            source_word: b.source_word,
            source,
            imported: *producer,
            result_id: *result_id,
            type_id: *type_id,
            set_id: *set_id,
            instruction: *instruction,
            operand: arguments[0],
            original_lanes: b.original_lanes.clone(),
            control: control_expected,
        });
    }
    // Every emitted controlled Floor must have come from a receipt binding.
    for e in emitted.floor.iter().filter(|e| e.control.is_some()) {
        if !out.iter().any(|j| j.source == e.producer) {
            return Err("RoundNiEmittedControlWithoutReceipt".into());
        }
    }
    Ok(out)
}
