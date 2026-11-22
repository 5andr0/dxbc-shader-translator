//! The viewer-side form of the precise SPIR-V artifact. The translator prepares it
//! (`prepare_precise_spirv`) and returns the words, digest, receipt summary, the payload the GPL
//! `ShaderPayload104::controlled` builds from it and the result of `validate_payload` over its words.
//! The imported naga module is re-imported here from the words by the same checked import as the
//! translator (patched naga, MIT/Apache); its Debug digest must equal the translator's.
use crate::{Error, sha256};
use serde::{Deserialize, Serialize};

/// The receipt summary the viewer reads (`identity()`, `records().len()`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrecisionReceipt {
    pub identity: String,
    pub records: Vec<String>,
}
impl PrecisionReceipt {
    pub fn identity(&self) -> &str { &self.identity }
    pub fn records(&self) -> &[String] { &self.records }
}

/// Wire form produced by the translator.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedSpirvWire {
    pub words: Vec<u32>,
    pub digest: String,
    pub receipt: PrecisionReceipt,
    pub r2_dropped_integer_no_contraction: Option<Vec<u32>>,
    pub imported_debug_sha256: String,
    pub validate_payload: Result<(), String>,
    pub controlled: Result<crate::payload104::ShaderPayload104, crate::Error>,
}

#[derive(Clone, Debug)]
pub struct PreparedSpirv {
    wire: PreparedSpirvWire,
    imported: naga::Module,
}
impl PreparedSpirv {
    pub fn from_wire(wire: PreparedSpirvWire) -> crate::Result<Self> {
        let options = naga::front::spv::Options { adjust_coordinate_space: false, strict_capabilities: true, block_ctx_dump_prefix: None };
        let (mut imported, report) = naga::front::spv::Frontend::new(wire.words.iter().copied(), &options)
            .parse_with_report()
            .map_err(|e| Error::Validation(e.to_string()))?;
        if let Some(dropped) = &wire.r2_dropped_integer_no_contraction {
            restore_r2_dropped_integers(&mut imported, &report, dropped).map_err(Error::Validation)?;
        }
        if sha256(format!("{imported:?}").as_bytes()) != wire.imported_debug_sha256 {
            return Err(Error::Io("PRETRANS1177 ERROR: precise module re-import differs from the translator's imported module".into()));
        }
        Ok(Self { wire, imported })
    }
    pub fn wire(&self) -> &PreparedSpirvWire { &self.wire }
    pub fn words(&self) -> &[u32] { &self.wire.words }
    pub fn payload_sha256(&self) -> &str { &self.wire.digest }
    pub fn receipt(&self) -> &PrecisionReceipt { &self.wire.receipt }
    pub fn imported_module(&self) -> &naga::Module { &self.imported }
    /// The translator's `validate_payload` over these words; other words cannot be checked here.
    pub fn validate_payload(&self, words: &[u32]) -> Result<(), String> {
        if words != self.wire.words.as_slice() {
            return Err("PRETRANS1177 ERROR: validate_payload over foreign words runs only in the translator".into());
        }
        self.wire.validate_payload.clone()
    }
}
fn stock_function(m: &naga::Module, r: naga::correspondence::ExpressionRef) -> &naga::Function {
    match r.owner {
        naga::correspondence::Owner::Function(h) => &m.functions[h],
        naga::correspondence::Owner::EntryPoint(i) => &m.entry_points[i].function,
    }
}
fn stock_type(
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
pub fn restore_r2_dropped_integers(
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
