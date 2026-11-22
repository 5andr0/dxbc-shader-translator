//! Exact unary instruction joins and opaque source-to-consumer endpoints, CPU only.
use iridia::precision::{ProducerRole, reference};
use iridia_naga as ir;
#[path = "../../../.patched/iridia-0.5.0-3cbe8f570937/src/precision/exact_eval.rs"]
#[allow(dead_code)] // The shared known-bit entry point is exercised by the MOV carrier.
mod imported_eval;
#[derive(Clone, Debug)]
pub struct JoinedUnary {
    pub source: ir::correspondence::ExpressionRef,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub set_id: u32,
    pub instruction: u32,
    pub operand: (u32, naga::correspondence::ExpressionRef),
    pub control: Option<naga::ArithmeticControl>,
}
#[derive(Clone, Debug)]
pub struct JoinedConsumer {
    pub source_word: usize,
    pub destination_lane: u8,
    pub consumer_word: usize,
    pub source_operand: usize,
    pub source_lane: u8,
    pub producer: naga::correspondence::ExpressionRef,
    pub operand: naga::correspondence::ExpressionRef,
    pub ordinal: usize,
    pub lane: u8,
    pub exp_result_id: u32,
    pub consumer_result_id: u32,
    pub operand_id: u32,
    pub analysis: imported_eval::Analysis,
}
pub(super) fn function(
    m: &naga::Module,
    r: naga::correspondence::ExpressionRef,
) -> &naga::Function {
    match r.owner {
        naga::correspondence::Owner::Function(h) => &m.functions[h],
        naga::correspondence::Owner::EntryPoint(i) => &m.entry_points[i].function,
    }
}
pub(super) fn join_one(
    source: ir::correspondence::ExpressionRef,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
) -> Result<JoinedUnary, String> {
    let es = emitted
        .exp2
        .iter()
        .filter(|e| e.producer == source)
        .collect::<Vec<_>>();
    if es.len() != 1 {
        return Err("ExpEmissionMissingOrAmbiguous".into());
    }
    let e = es[0];
    let rows = report
        .exp2
        .iter()
        .filter(|r| match r {
            naga::correspondence::ImportedExp2::Direct { result_id, .. }
            | naga::correspondence::ImportedExp2::Unsupported { result_id, .. } => {
                *result_id == e.result_id
            }
        })
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err("ExpImportMissingOrAmbiguous".into());
    }
    let naga::correspondence::ImportedExp2::Direct {
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
        return Err("ExpImportAdaptedPhiSpillUnsupported".into());
    };
    let expected = e.control.map(|c| match c {
        ir::ArithmeticControl::Exp2F32NoRefactoringV1 => {
            naga::ArithmeticControl::Exp2F32NoRefactoringV1
        }
        ir::ArithmeticControl::NoContraction => naga::ArithmeticControl::NoContraction,
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
    });
    if *type_id != e.type_id
        || *set_id != e.set_id
        || *instruction != 29
        || e.instruction != 29
        || arguments[0].0 != e.arguments[0].1
        || arguments[0].1.owner != producer.owner
        || e.arguments[0].0.owner != source.owner
        || *control != expected
    {
        return Err("ExpIdsOwnerControlMismatch".into());
    }
    let f = function(imported, *producer);
    if !matches!(f.expressions[producer.expression],naga::Expression::Math{fun:naga::MathFunction::Exp2,arg,arg1:None,arg2:None,arg3:None} if arg==arguments[0].1.expression)
        || f.expression_controls.get(&producer.expression).copied() != expected
    {
        return Err("ExpActualOperationControlMismatch".into());
    }
    for r in [*producer, arguments[0].1] {
        if super::precision_carrier::stock_type(imported, r)? != (naga::ScalarKind::Float, 4, 1) {
            return Err("ExpActualScalarTypeMismatch".into());
        }
    }
    Ok(JoinedUnary {
        source,
        imported: *producer,
        result_id: *result_id,
        type_id: *type_id,
        set_id: *set_id,
        instruction: *instruction,
        operand: arguments[0],
        control: *control,
    })
}
pub(super) fn join(
    receipt: &iridia::precision::PrecisionReceipt,
    imported: &naga::Module,
    emitted: &ir::correspondence::EmissionReport,
    report: &naga::correspondence::ImportReport,
    binary: &[super::precision_carrier::JoinedProducer],
    raw_roots582: &[naga::Handle<naga::GlobalVariable>],
    samples648:&[crate::sample_import648::JoinedSample648],
) -> Result<(Vec<JoinedUnary>, Vec<JoinedConsumer>), String> {
    let mut unary = vec![];
    let mut consumers = vec![];
    for b in receipt
        .bindings()
        .iter()
        .filter(|b| b.role == ProducerRole::Exp2Scalar)
    {
        let j = join_one(reference(&b.producer), imported, emitted, report)?;
        let e = emitted
            .exp2
            .iter()
            .find(|e| e.producer == j.source)
            .unwrap();
        if e.arguments[0].0 != reference(&b.operands[0]) || e.control != b.control() {
            return Err("ExpSourceOperandControlMismatch".into());
        }
        let p = receipt
            .unary_proofs()
            .iter()
            .find(|p| p.producer == b.producer)
            .ok_or("ExpSourceProofMissing")?;
        let analysis = imported_eval::analyze_imported_sampled648(
            imported,
            j.imported.owner,
            &[],
            &[j.imported.expression],
            raw_roots582,
            &crate::sample_import648::results_for(samples648,j.imported.owner),
        )?;
        let actual = analysis
            .uses
            .iter()
            .filter(|u| u.atom.opaque.is_some())
            .collect::<Vec<_>>();
        let mut matched = vec![];
        for u in &p.uses {
            let c = binary
                .iter()
                .find(|c| {
                    c.source.owner == b.producer.owner
                        && c.source.expression == u.edge.producer
                        && c.controlled
                })
                .ok_or("ExpConsumerJoinMissing")?;
            if c.imported.owner != j.imported.owner {
                return Err("ExpConsumerOwnerMismatch".into());
            }
            let a = c
                .operands
                .get(u.edge.ordinal)
                .ok_or("ExpConsumerOrdinalMismatch")?;
            let indices = actual
                .iter()
                .enumerate()
                .filter(|(_, v)| {
                    v.producer == c.imported.expression
                        && v.operand == a.1.expression
                        && v.ordinal == u.edge.ordinal
                        && v.lane == u.edge.lane
                        && v.atom.opaque == Some(j.imported.expression)
                        && v.atom.bits.is_none()
                })
                .map(|(k, _)| k)
                .collect::<Vec<_>>();
            if indices.len() != 1 || matched.contains(&indices[0]) {
                return Err("ExpImportedReachingUseMismatch".into());
            }
            matched.push(indices[0]);
            consumers.push(JoinedConsumer {
                source_word: p.source_word,
                destination_lane: p.destination_lane,
                consumer_word: u.source_word,
                source_operand: u.source_operand,
                source_lane: u.source_lane,
                producer: c.imported,
                operand: a.1,
                ordinal: u.edge.ordinal,
                lane: u.edge.lane,
                exp_result_id: j.result_id,
                consumer_result_id: c.result_id,
                operand_id: a.0,
                analysis: analysis.clone(),
            });
        }
        if actual.len() != matched.len() {
            return Err("ExpUnjoinedImportedUse".into());
        }
        unary.push(j);
    }
    Ok((unary, consumers))
}
/// Transport never supplies numerical or runtime admission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QualificationPlan {
    pub first_target: &'static str,
    pub unresolved: [QualificationGate; 5],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QualificationGate {
    SourceFullDomainSpecialValuesAndFtz,
    DxcCompilerProfileAndNumerics,
    VulkanAccuracyAndDeviceEnablement,
    SourceTargetResourceHostGeneration,
    RendererCacheReflectionAdmission,
}
impl Default for QualificationPlan {
    fn default() -> Self {
        Self {
            first_target: "Windows D3D12 / pinned DXC",
            unresolved: [
                QualificationGate::SourceFullDomainSpecialValuesAndFtz,
                QualificationGate::DxcCompilerProfileAndNumerics,
                QualificationGate::VulkanAccuracyAndDeviceEnablement,
                QualificationGate::SourceTargetResourceHostGeneration,
                QualificationGate::RendererCacheReflectionAdmission,
            ],
        }
    }
}
