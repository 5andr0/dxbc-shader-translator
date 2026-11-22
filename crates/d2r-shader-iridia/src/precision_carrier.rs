//! CPU-only checked interchange. This artifact grants no cache, renderer or device admission.
use iridia::precision::{PrecisionReceipt, module_identity, reference};
use iridia_naga as ir;
use sha2::{Digest, Sha256};
const STOCK: &str = "95ccfe0995d3a31d39edc6208039bd44d8bbe14a998fa2340ea1a5036bc8ee82";
const PRIMASE: &str = "8cbf6658602ecb8f480292ac77f6fc9155b9f9462d23019318be73777dbdbe10";
fn digest(words: &[u32]) -> String {
    format!(
        "{:x}",
        Sha256::digest(
            words
                .iter()
                .flat_map(|w| w.to_le_bytes())
                .collect::<Vec<_>>()
        )
    )
}
/// Internal Naga-to-Naga interchange preserves the source IR's clip/depth values.
/// These words are reimported with coordinate adjustment disabled and submitted
/// as Naga IR, not sent directly to a Vulkan driver. The final backend owns its
/// target coordinate conversion and fragment-depth policy.
///
/// The writer flags are stated in full. `spv::Options::default()` adds
/// `WriterFlags::DEBUG` (OpName/OpMemberName) only under `cfg!(debug_assertions)`
/// of the Naga crate, so inheriting them made release builds emit nameless words
/// and refuse every original graphics pair whose linkage is tied to reimported
/// names. The qualified (debug-built) interchange carries the debug names.
pub(crate) fn internal_spirv_options() -> ir::back::spv::Options<'static> {
    ir::back::spv::Options {
        lang_version: (1, 2),
        flags: INTERNAL_SPIRV_FLAGS,
        capabilities: None,
        ..Default::default()
    }
}
/// Build-profile independent writer flags of the internal interchange.
pub(crate) const INTERNAL_SPIRV_FLAGS: ir::back::spv::WriterFlags =
    ir::back::spv::WriterFlags::LABEL_VARYINGS.union(ir::back::spv::WriterFlags::DEBUG);
/// The stock writer configuration of the earlier transport fixtures (coordinate and
/// depth adjustment kept), with the debug-build flags stated so every profile matches.
pub(crate) fn stock_debug_spirv_options() -> ir::back::spv::Options<'static> {
    ir::back::spv::Options {
        lang_version: (1, 2),
        flags: INTERNAL_SPIRV_FLAGS
            .union(ir::back::spv::WriterFlags::ADJUST_COORDINATE_SPACE)
            .union(ir::back::spv::WriterFlags::CLAMP_FRAG_DEPTH),
        ..Default::default()
    }
}
#[derive(Clone, Debug)]
pub struct JoinedProducer {
    pub source_word: usize,
    pub source: iridia::precision::OwnedExpression,
    pub imported: naga::correspondence::ExpressionRef,
    pub result_id: u32,
    pub type_id: u32,
    pub opcode: u32,
    pub operands: [(u32, naga::correspondence::ExpressionRef); 2],
    pub controlled: bool,
    /// D3D source modifier per ordered operand, checked in source, words and import.
    pub modifiers: Vec<u32>,
}
#[derive(Clone, Debug)]
pub struct PreparedSpirv {
    samples648: Vec<crate::sample_import648::JoinedSample648>,
    round_ni849: Vec<crate::round_ni_carrier849::JoinedRoundNi>,
    less865: Vec<crate::compare_carrier865::JoinedLess865>,
    frac865: Vec<crate::frac_carrier865d::JoinedFrac865>,
    tolerance865: Vec<crate::tolerance_carrier865c::JoinedTolerance865>,
    add_sat865: Vec<crate::sat_carrier865d::JoinedAddSat865>,
    raw582: Vec<crate::raw_carrier582::JoinedRaw582>,
    bit_exact: Vec<crate::bit_carrier575::JoinedBits575>,
    unary: Vec<crate::unary_carrier::JoinedUnary>,
    consumers: Vec<crate::unary_carrier::JoinedConsumer>,
    qualification: crate::unary_carrier::QualificationPlan,
    words: Vec<u32>,
    digest: String,
    prepared: iridia::emit::PreparedModule,
    imported: naga::Module,
    emitted: ir::correspondence::EmissionReport,
    import_report: naga::correspondence::ImportReport,
    joined: Vec<JoinedProducer>,
    selection: Option<crate::selection584::SelectionProof584>,
    exact_joined: Vec<crate::exact_move_carrier::JoinedExactUse>,
    module_id: String,
    imported_id: String,
    receipt_id: String,
    report_id: String,
    join_id: String,
    stage: ir::ShaderStage,
    entry: String,
}
impl PreparedSpirv {
    pub(crate) fn selection_evidence584(&self) -> Option<String> {
        self.selection.as_ref().map(|p| p.evidence())
    }
    pub fn raw582(&self)->&[crate::raw_carrier582::JoinedRaw582]{&self.raw582}
    pub fn samples648(&self)->&[crate::sample_import648::JoinedSample648]{&self.samples648}
    pub fn round_ni849(&self) -> &[crate::round_ni_carrier849::JoinedRoundNi] { &self.round_ni849 }
    pub fn less865(&self) -> &[crate::compare_carrier865::JoinedLess865] { &self.less865 }
    pub fn frac865(&self) -> &[crate::frac_carrier865d::JoinedFrac865] { &self.frac865 }
    pub fn tolerance865(&self) -> &[crate::tolerance_carrier865c::JoinedTolerance865] { &self.tolerance865 }
    pub fn add_sat865(&self) -> &[crate::sat_carrier865d::JoinedAddSat865] { &self.add_sat865 }
    /// Execution-local inspection only; artifacts authenticate reports and modules.
    pub fn emission_report648(&self)->&ir::correspondence::EmissionReport{&self.emitted}
    pub fn import_report648(&self)->&naga::correspondence::ImportReport{&self.import_report}
    pub fn bit_exact(&self) -> &[crate::bit_carrier575::JoinedBits575] { &self.bit_exact }
    pub fn unary(&self) -> &[crate::unary_carrier::JoinedUnary] {
        &self.unary
    }
    pub fn consumers(&self) -> &[crate::unary_carrier::JoinedConsumer] {
        &self.consumers
    }
    pub fn qualification(&self) -> &crate::unary_carrier::QualificationPlan {
        &self.qualification
    }

    pub fn exact_joined(&self) -> &[crate::exact_move_carrier::JoinedExactUse] {
        &self.exact_joined
    }
    pub fn words(&self) -> &[u32] {
        &self.words
    }
    pub fn payload_sha256(&self) -> &str {
        &self.digest
    }
    pub fn receipt(&self) -> &PrecisionReceipt {
        &self.prepared.precision
    }
    pub fn prepared_module(&self) -> &ir::Module {
        &self.prepared.module
    }
    pub fn imported_module(&self) -> &naga::Module {
        &self.imported
    }
    pub fn joined(&self) -> &[JoinedProducer] {
        &self.joined
    }
    /// Exact pinned full-package maps: SHA-256 of sorted compact JSON mapping
    /// relative POSIX paths to raw-byte SHA-256 values, including notices/metadata.
    /// Returns (Primase, stock); labels grant no consumed-build or runtime admission.
    pub fn package_identities(&self) -> (&'static str, &'static str) {
        (PRIMASE, STOCK)
    }
    pub fn validate_payload(&self, words: &[u32]) -> Result<(), String> {
        if digest(words) != self.digest {
            return Err("PrecisionPayloadDigestMismatch".into());
        }
        if iridia::source_hash(
            format!(
                "{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}{:?}",
                self.joined, self.exact_joined, self.unary, self.consumers, self.qualification, self.bit_exact, self.raw582, self.selection, self.samples648, self.round_ni849, self.less865, self.frac865, self.tolerance865, self.add_sat865
            )
            .as_bytes(),
        ) != self.join_id
            || module_identity(&self.prepared.module) != self.module_id
            || iridia::source_hash(format!("{:?}", self.imported).as_bytes()) != self.imported_id
            || self.prepared.precision.identity() != self.receipt_id
            || iridia::source_hash(format!("{:?}{:?}", self.emitted, self.import_report).as_bytes())
                != self.report_id
        {
            return Err("PrecisionStaleCorrespondence".into());
        }
        self.prepared.precision.validate(
            &self.prepared.module,
            self.prepared
                .precision
                .records()
                .first()
                .map(|r| r.source_sha.as_str()),
        )?;
        Ok(())
    }
    pub fn stage_entry(&self) -> (ir::ShaderStage, &str) {
        (self.stage, &self.entry)
    }
}
fn stock_function(m: &naga::Module, r: naga::correspondence::ExpressionRef) -> &naga::Function {
    match r.owner {
        naga::correspondence::Owner::Function(h) => &m.functions[h],
        naga::correspondence::Owner::EntryPoint(i) => &m.entry_points[i].function,
    }
}
pub(super) fn stock_type(
    m: &naga::Module,
    r: naga::correspondence::ExpressionRef,
) -> Result<(naga::ScalarKind, u8, u8), String> {
    let f = stock_function(m, r);
    let mut t = naga::front::Typifier::new();
    t.grow(
        r.expression,
        &f.expressions,
        &naga::proc::ResolveContext::with_locals(m, &f.local_variables, &f.arguments),
    )
    .map_err(|e| e.to_string())?;
    match t.get(r.expression, &m.types) {
        naga::TypeInner::Scalar(s) => Ok((s.kind, s.width, 1)),
        naga::TypeInner::Vector { size, scalar } => Ok((scalar.kind, scalar.width, *size as u8)),
        _ => Err("unsupported carrier type".into()),
    }
}
/// R2 (carrier rule 4): restores the integer `NoContraction` controls that the writer
/// dropped from an R2 module (no legal SPIR-V form: NoContraction is invalid with FPFastMathDefault,
/// FPFastMathMode is float-only), from that writer execution's record `dropped` (result ids).
/// Every id must name exactly one direct imported integer Add/Subtract/Multiply without a control;
/// the restored module is revalidated. Shared by `prepare_spirv` and every payload re-import.
pub(crate) fn restore_r2_dropped_integers(
    imported: &mut naga::Module,
    import_report: &naga::correspondence::ImportReport,
    dropped: &[u32],
) -> Result<(), String> {
    if import_report.r2_865.is_none() {
        return Err("R2DroppedIntegerWithoutR2Import".into());
    }
    for id in dropped {
        let rows: Vec<_> = import_report
            .arithmetic
            .iter()
            .filter_map(|r| match r {
                naga::correspondence::ImportedArithmetic::Direct { result_id, producer, .. } if result_id == id => Some(*producer),
                _ => None,
            })
            .collect();
        let [producer] = rows[..] else { return Err("R2DroppedIntegerImportMissing".into()) };
        let integer = stock_type(imported, producer)
            .is_ok_and(|t| matches!(t.0, naga::ScalarKind::Uint | naga::ScalarKind::Sint));
        let f = match producer.owner {
            naga::correspondence::Owner::Function(h) => &mut imported.functions[h],
            naga::correspondence::Owner::EntryPoint(k) => &mut imported.entry_points[k].function,
        };
        let shape = matches!(f.expressions[producer.expression], naga::Expression::Binary {
            op: naga::BinaryOperator::Add | naga::BinaryOperator::Subtract | naga::BinaryOperator::Multiply, .. });
        if !integer || !shape
            || f.expression_controls.insert(producer.expression, naga::ArithmeticControl::NoContraction).is_some()
        {
            return Err("R2DroppedIntegerRestoreMismatch".into());
        }
    }
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(imported)
        .map_err(|e| e.to_string())?;
    Ok(())
}
/// Runs the same resource/interface pipeline, actual compaction, writer/report and safe parser/report.
pub fn prepare_spirv(
    program: &iridia::Program,
    pipeline: &iridia::emit::Pipeline,
) -> Result<PreparedSpirv, String> {
    let mut prepared =
        iridia::emit::prepare_module(program, pipeline).map_err(|e| e.to_string())?;
    if !prepared.notes.is_empty() {
        return Err(format!(
            "unapproved pipeline transformations: {:?}",
            prepared.notes
        ));
    }
    if !prepared.module.overrides.is_empty() {
        return Err("PreciseReceiptRemapUnavailable: specialized overrides".into());
    }
    let (module, receipt, report) = prepared
        .precision
        .compact(&prepared.module, ir::compact::KeepUnused::Yes)?;
    // Translate resource handles through the same execution; no assumption that they stayed numbered alike.
    for (id, _) in &prepared.bindings {
        let old = program.global(*id).ok_or("missing original resource")?;
        if !report
            .globals
            .iter()
            .any(|r| r.old == old && matches!(r.new, ir::correspondence::Disposition::Survives(_)))
        {
            return Err("prepared resource removed".into());
        }
    }
    prepared.module = module;
    prepared.precision = receipt;
    let selection_required = prepared.precision.is_precision_bearing()
        && crate::selection584::preflight(&prepared.module)?;
    let info = ir::valid::Validator::new(
        ir::valid::ValidationFlags::all(),
        ir::valid::Capabilities::all(),
    )
    .validate(&prepared.module)
    .map_err(|e| e.to_string())?;
    let ep = prepared
        .module
        .entry_points
        .first()
        .ok_or("carrier requires one entry")?;
    if prepared.module.entry_points.len() != 1 {
        return Err("unsupported multi-entry carrier lifecycle".into());
    }
    let stage = ep.stage;
    let entry = ep.name.clone();
    let (words, emitted) = ir::back::spv::write_vec_with_report(
        &prepared.module,
        &info,
        &internal_spirv_options(),
        // The module is checked single-entry above. None preserves its unused
        // declarations for final resource/projection reflection; selecting that
        // same entry would silently omit globals with empty entry usage.
        None,
    )
    .map_err(|e| e.to_string())?;
    let options = naga::front::spv::Options {
        adjust_coordinate_space: false,
        strict_capabilities: true,
        block_ctx_dump_prefix: None,
    };
    let (mut imported, import_report) =
        naga::front::spv::Frontend::new(words.iter().copied(), &options)
            .parse_with_report()
            .map_err(|e| e.to_string())?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&imported)
    .map_err(|e| e.to_string())?;
    // R2 carrier. The writer replaced float NoContraction by FPFastMathMode
    // 0x7 (imported back as NoContraction) and dropped integer NoContraction; restore exactly the
    // dropped integer controls from this writer execution's record so the import round-trips.
    match (emitted.r2_865.as_ref(), import_report.r2_865.as_ref()) {
        (None, None) => {}
        (Some(e), Some(i)) => {
            if e.fp_fast_math_0x7 != i.fp_fast_math_0x7 || i.defaults.iter().map(|d| d.0).collect::<Vec<_>>() != e.entry_ids {
                return Err("R2CarrierImportMismatch".into());
            }
            restore_r2_dropped_integers(&mut imported, &import_report, &e.dropped_integer_no_contraction)?;
        }
        _ => return Err("R2CarrierOneSided".into()),
    }
    if emitted.revision != 2 || import_report.revision != 2 {
        return Err("correspondence revision mismatch".into());
    }
    let selection = selection_required.then(|| crate::selection584::join(
        &prepared.module, &imported, &emitted, &import_report,
    )).transpose()?;
    let mut joined = vec![];
    let spirv_defs = crate::modifier_carrier849b::defs(&words)?;
    for b in prepared
        .precision
        .bindings()
        .iter()
        .filter(|b| !matches!(b.role, iridia::precision::ProducerRole::SampleExplicitLod648 | iridia::precision::ProducerRole::Exp2Scalar | iridia::precision::ProducerRole::RoundNi | iridia::precision::ProducerRole::CompareLess | iridia::precision::ProducerRole::MaxCompare | iridia::precision::ProducerRole::SaturateAdd | iridia::precision::ProducerRole::ToleranceSin | iridia::precision::ProducerRole::ToleranceCos | iridia::precision::ProducerRole::ToleranceRsq | iridia::precision::ProducerRole::ToleranceDiv | iridia::precision::ProducerRole::FracFloor | iridia::precision::ProducerRole::FracSubtract | iridia::precision::ProducerRole::RawLoad582 | iridia::precision::ProducerRole::RegisterCopy | iridia::precision::ProducerRole::ConditionalMove | iridia::precision::ProducerRole::LogicalShift))
    {
        let ir::correspondence::ArithmeticEmission::Direct(e) =
            emitted.producer(reference(&b.producer))
        else {
            return Err("PrecisionEmissionNotDirect".into());
        };
        let row = import_report
            .arithmetic
            .iter()
            .find(|i| match i {
                naga::correspondence::ImportedArithmetic::Direct { result_id, .. }
                | naga::correspondence::ImportedArithmetic::Unsupported { result_id, .. } => {
                    *result_id == e.result_id
                }
            })
            .ok_or("PrecisionImportMissing")?;
        let naga::correspondence::ImportedArithmetic::Direct {
            result_id,
            type_id,
            producer,
            arguments,
            ..
        } = row
        else {
            return Err("PrecisionImportUnsupportedAdaptation".into());
        };
        if *type_id != e.type_id {
            return Err("PrecisionCarrierTypeIdMismatch".into());
        }
        for k in 0..2 {
            if e.arguments[k].0 != reference(&b.operands[k]) || e.arguments[k].1 != arguments[k].0 {
                return Err("PrecisionCarrierOrderedOperandMismatch".into());
            }
        }
        let kind = match b.kind {
            iridia::build::Kind::F => naga::ScalarKind::Float,
            iridia::build::Kind::U => naga::ScalarKind::Uint,
            _ => return Err("carrier kind".into()),
        };
        for (r, width) in std::iter::once((*producer, b.width)).chain(
            arguments
                .iter()
                .zip(&b.input_widths)
                .map(|(a, w)| (a.1, *w)),
        ) {
            if stock_type(&imported, r)? != (kind, b.scalar_bytes, width.get()) {
                return Err("PrecisionCarrierTypeMismatch".into());
            }
        }
        let f = stock_function(&imported, *producer);
        let args = [arguments[0].1.expression, arguments[1].1.expression];
        if arguments.iter().any(|a| a.1.owner != producer.owner) {
            return Err("PrecisionCarrierOwnerMismatch".into());
        }
        let source = prepared
            .precision
            .records()
            .iter()
            .find(|r| r.word == b.source_word)
            .ok_or("source missing")?;
        let (valid, opcode) = match (
            &f.expressions[producer.expression],
            source.instruction.name(),
        ) {
            (
                naga::Expression::Binary { op, left, right },
                name @ ("add" | "iadd" | "mul" | "imul" | "umul"),
            ) => {
                let add = matches!(name, "add" | "iadd");
                (
                    *op == if add {
                        naga::BinaryOperator::Add
                    } else {
                        naga::BinaryOperator::Multiply
                    } && args == [*left, *right],
                    match (add, kind) {
                        (true, naga::ScalarKind::Float) => 129,
                        (false, naga::ScalarKind::Float) => 133,
                        (true, _) => 128,
                        (false, _) => 132,
                    },
                )
            }
            (
                naga::Expression::Math {
                    fun: naga::MathFunction::Dot,
                    arg,
                    arg1: Some(right),
                    arg2: None,
                    arg3: None,
                },
                "dp2" | "dp3" | "dp4",
            ) => (args == [*arg, *right], 148),
            _ => (false, 0),
        };
        if !valid
            || e.opcode != opcode
            || f.expression_controls.get(&producer.expression).copied()
                != b.controlled
                    .then_some(naga::ArithmeticControl::NoContraction)
        {
            return Err("PrecisionCarrierOperationOrControlMismatch".into());
        }
        let modifiers = iridia::precision::source_modifiers(source).map_err(|e| e.to_string())?;
        if modifiers.len() != 2
            || (0..2).any(|k| {
                spirv_defs.modifier(e.arguments[k].1) != modifiers[k]
                    || crate::modifier_carrier849b::imported_modifier(f, arguments[k].1.expression)
                        != modifiers[k]
            })
        {
            return Err("PrecisionCarrierSourceModifierMismatch".into());
        }
        joined.push(JoinedProducer {
            source_word: b.source_word,
            source: b.producer.clone(),
            imported: *producer,
            result_id: *result_id,
            type_id: *type_id,
            opcode,
            operands: *arguments,
            controlled: b.controlled,
            modifiers,
        });
    }
    let raw582=crate::raw_carrier582::join(&prepared.module,&prepared.precision,&emitted,&words,&imported)?;
    let raw_roots582=crate::raw_carrier582::roots(&raw582);
    let samples648=crate::sample_import648::join(&prepared.module,&prepared.precision,&emitted,&words,&imported,&import_report,&joined,&raw_roots582)?;
    let (unary, consumers) = crate::unary_carrier::join(
        &prepared.precision,
        &imported,
        &emitted,
        &import_report,
        &joined,
        &raw_roots582,
        &samples648,
    )?;
    let round_ni849 =
        crate::round_ni_carrier849::join(&prepared.precision, &imported, &emitted, &import_report)?;
    let add_sat865 = crate::sat_carrier865d::join(&prepared.precision, &imported, &emitted, &import_report)?;
    let tolerance865 = crate::tolerance_carrier865c::join(&prepared.precision, &imported, &emitted, &import_report)?;
    let frac865 = crate::frac_carrier865d::join(
        &prepared.precision, &imported, &emitted, &import_report, &round_ni849,
    )?;
    let less865 =
        crate::compare_carrier865::join(&prepared.precision, &imported, &emitted, &import_report)?;
    for (owner, f) in imported
        .functions
        .iter()
        .map(|(h, f)| (naga::correspondence::Owner::Function(h), f))
        .chain(
            imported
                .entry_points
                .iter()
                .enumerate()
                .map(|(i, e)| (naga::correspondence::Owner::EntryPoint(i), &e.function)),
        )
    {
        for h in f.expression_controls.keys() {
            if !joined
                .iter()
                .any(|j| j.controlled && j.imported.owner == owner && j.imported.expression == *h)
                && !unary.iter().any(|j| {
                    j.control.is_some() && j.imported.owner == owner && j.imported.expression == *h
                })
                && !round_ni849
                    .iter()
                    .any(|j| j.imported.owner == owner && j.imported.expression == *h)
                && !add_sat865
                    .iter()
                    .any(|j| j.imported.owner == owner && j.imported.expression == *h)
                && !tolerance865
                    .iter()
                    .any(|j| j.imported.owner == owner && j.imported.expression == *h)
                && !frac865
                    .iter()
                    .any(|j| j.imported.owner == owner && j.imported.expression == *h)
                && !less865
                    .iter()
                    .any(|j| j.imported.owner == owner && j.imported.expression == *h)
            {
                return Err("PrecisionImportedControlWithoutSource".into());
            }
        }
    }
    let exact_joined =
        crate::exact_move_carrier::join(&prepared.precision, &imported, &emitted, &import_report, &raw_roots582,&samples648)?;
    let bit_exact = crate::bit_carrier575::join(&prepared.precision, &emitted, &words)?;
    let qualification = crate::unary_carrier::QualificationPlan::default();
    let artifact = PreparedSpirv {
        join_id: iridia::source_hash(
            format!("{joined:?}{exact_joined:?}{unary:?}{consumers:?}{qualification:?}{bit_exact:?}{raw582:?}{selection:?}{samples648:?}{round_ni849:?}{less865:?}{frac865:?}{tolerance865:?}{add_sat865:?}").as_bytes(),
        ),
        round_ni849,
        less865,
        frac865,
        tolerance865,
        add_sat865,
        selection,
        exact_joined,
        bit_exact,
        raw582,
        samples648,
        unary,
        consumers,
        qualification,
        digest: digest(&words),
        module_id: module_identity(&prepared.module),
        imported_id: iridia::source_hash(format!("{imported:?}").as_bytes()),
        receipt_id: prepared.precision.identity(),
        report_id: iridia::source_hash(format!("{emitted:?}{import_report:?}").as_bytes()),
        words,
        prepared,
        imported,
        emitted,
        import_report,
        joined,
        stage,
        entry,
    };
    artifact.validate_payload(artifact.words())?;
    Ok(artifact)
}

/// Opt-in CPU preparation for an already lifted pair. Existing caller/resource and link checks remain mandatory.
/// This intentionally does not produce PreparedShader or grant production/source-profile admission.
pub fn prepare_pair(
    vs: &iridia::Program,
    ps: &iridia::Program,
    vertex: &crate::resource_contract::CallerContract,
    fragment: &crate::resource_contract::CallerContract,
    draw: &crate::stage_link::DrawContract,
) -> Result<(PreparedSpirv, PreparedSpirv), String> {
    if vertex.resources.iter().chain(&fragment.resources).any(|r| {
        matches!(
            r.resource,
            crate::resource_contract::ResourceContractKind::UnreadDeclaration { .. }
        )
    }) {
        return Err("CPU carrier pair does not admit unread caller projections".into());
    }
    let mut v = vs.clone();
    let mut p = ps.clone();
    crate::sample_lod::repair_exact_lod(&mut v).map_err(|e| e.to_string())?;
    crate::sample_lod::repair_exact_lod(&mut p).map_err(|e| e.to_string())?;
    crate::resource_contract::apply_resources(&mut v, vertex)?;
    crate::resource_contract::apply_resources(&mut p, fragment)?;
    if !v.notes.is_empty() || !p.notes.is_empty() {
        return Err("unresolved source notes".into());
    }
    crate::stage_link::apply_draw_contract(&mut v, draw)?;
    crate::stage_link::check_link(&v, &p, &draw.topology)?;
    let vs = prepare_spirv(
        &v,
        &iridia::emit::Pipeline {
            bindings: crate::resource_contract::pipeline_bindings(&v),
            feeds: Some(&p.interface),
            ..Default::default()
        },
    )?;
    let ps = prepare_spirv(
        &p,
        &iridia::emit::Pipeline {
            bindings: crate::resource_contract::pipeline_bindings(&p),
            ..Default::default()
        },
    )?;
    Ok((vs, ps))
}

