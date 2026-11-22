//! Exact-bit proof at the same actual arithmetic operand, not a store/load ID guess.
use iridia_naga as ir;
#[path = "../../../.patched/iridia-0.5.0-3cbe8f570937/src/precision/exact_eval.rs"]
mod imported_eval;
#[derive(Clone, Debug)]
pub struct JoinedExactUse {
    pub source_word: usize,
    pub destination_lane: u8,
    pub bits: u32,
    pub source_owner: iridia::precision::FunctionOwner,
    pub source_producer: ir::Handle<ir::Expression>,
    pub imported_producer: naga::correspondence::ExpressionRef,
    pub imported_operand: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub operand_id: u32,
    pub ordinal: usize,
    pub lane: u8,
    pub imported_assignments: Vec<imported_eval::Assignment>,
}
pub fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
    raw_roots582: &[naga::Handle<naga::GlobalVariable>],
    samples648:&[crate::sample_import648::JoinedSample648],
) -> Result<Vec<JoinedExactUse>, String> {
    let mut joined = vec![];
    for proof in receipt.exact_proofs() {
        for u in &proof.analysis.uses {
            let Some((source_word, destination_lane)) = u.atom.origin else {
                continue;
            };
            let source = iridia::precision::OwnedExpression {
                module_revision: receipt.revision(),
                owner: proof.owner.clone(),
                expression: u.producer,
            };
            if emitted
                .exp2
                .iter()
                .any(|e| e.producer == iridia::precision::reference(&source))
            {
                let e = emitted
                    .exp2
                    .iter()
                    .find(|e| e.producer == iridia::precision::reference(&source))
                    .unwrap();
                let j = crate::unary_carrier::join_one(
                    iridia::precision::reference(&source),
                    imported,
                    emitted,
                    report,
                )?;
                if u.ordinal != 0 || u.lane != 0 || e.arguments[0].0.expression != u.operand {
                    return Err("ExactMoveUnaryOperandMismatch".into());
                }
                let analysis = imported_eval::analyze_imported_sampled648(imported, j.imported.owner, &[], &[], raw_roots582,&crate::sample_import648::results_for(samples648,j.imported.owner))?;
                let values = analysis
                    .uses
                    .iter()
                    .filter(|a| {
                        a.producer == j.imported.expression
                            && a.operand == j.operand.1.expression
                            && a.ordinal == 0
                            && a.lane == 0
                    })
                    .collect::<Vec<_>>();
                if values.len() != 1 || values[0].atom.bits != u.atom.bits || u.atom.bits.is_none()
                {
                    return Err("ExactMoveUnaryBitsMismatch".into());
                }
                joined.push(JoinedExactUse {
                    source_word,
                    destination_lane,
                    bits: u.atom.bits.unwrap(),
                    source_owner: proof.owner.clone(),
                    source_producer: u.producer,
                    imported_producer: j.imported,
                    imported_operand: j.operand.1,
                    result_id: j.result_id,
                    type_id: j.type_id,
                    operand_id: j.operand.0,
                    ordinal: 0,
                    lane: 0,
                    imported_assignments: analysis.assignments,
                });
                continue;
            }
            let ir::correspondence::ArithmeticEmission::Direct(e) =
                emitted.producer(iridia::precision::reference(&source))
            else {
                return Err("ExactMoveEmissionNotDirectUnsupported".into());
            };
            if u.ordinal >= 2
                || e.arguments[u.ordinal].0.expression != u.operand
                || e.arguments[u.ordinal].0.owner != e.producer.owner
            {
                return Err("ExactMoveWriterOperandMismatch".into());
            }
            let rows: Vec<_> = report
                .arithmetic
                .iter()
                .filter(|r| match r {
                    naga::correspondence::ImportedArithmetic::Direct { result_id, .. }
                    | naga::correspondence::ImportedArithmetic::Unsupported { result_id, .. } => {
                        *result_id == e.result_id
                    }
                })
                .collect();
            if rows.len() != 1 {
                return Err("ExactMoveImportMissingOrAmbiguous".into());
            }
            let naga::correspondence::ImportedArithmetic::Direct {
                producer,
                arguments,
                type_id,
                ..
            } = rows[0]
            else {
                return Err("ExactMoveImportAdaptedPhiSpillUnsupported".into());
            };
            if *type_id != e.type_id
                || arguments
                    .iter()
                    .enumerate()
                    .any(|(k, a)| a.0 != e.arguments[k].1 || a.1.owner != producer.owner)
            {
                return Err("ExactMoveImportOwnerOperandMismatch".into());
            }
            let analysis = imported_eval::analyze_imported_sampled648(imported, producer.owner, &[], &[], raw_roots582,&crate::sample_import648::results_for(samples648,producer.owner))?;
            let values: Vec<_> = analysis
                .uses
                .iter()
                .filter(|a| {
                    a.producer == producer.expression
                        && a.operand == arguments[u.ordinal].1.expression
                        && a.ordinal == u.ordinal
                        && a.lane == u.lane
                })
                .collect();
            if values.len() != 1 || values[0].atom.bits != u.atom.bits {
                return Err("ExactMoveImportedReachingBitsMismatch".into());
            }
            joined.push(JoinedExactUse {
                source_word,
                destination_lane,
                bits: u.atom.bits.ok_or("ExactMoveUnknownBits")?,
                source_owner: proof.owner.clone(),
                source_producer: u.producer,
                imported_producer: *producer,
                imported_operand: arguments[u.ordinal].1,
                result_id: e.result_id,
                type_id: *type_id,
                operand_id: arguments[u.ordinal].0,
                ordinal: u.ordinal,
                lane: u.lane,
                imported_assignments: analysis.assignments,
            });
        }
    }
    Ok(joined)
}
