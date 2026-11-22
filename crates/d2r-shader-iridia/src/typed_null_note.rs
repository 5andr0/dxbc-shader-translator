//! Resolve only the source typed-format obligation made unnecessary by sealed
//! original NULL-result projection. This is not a resource-format conversion.
use crate::caller_projection::{CALLER_REVISION, CallerCapability, CallerProjectionReport};
use crate::resource_contract::{
    CallerContract, RegisterClass, ResolvedNote, ResourceContractKind, UnreadReason,
    apply_resources, source_declaration_sha256,
};
use iridia::{BufferKind, D3dClass, Program, ResourceKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const TYPED_NULL_NOTE_REVISION: &str = "d2r-iridia-source-matched-null-typed-note52-v1";
const TYPED_NOTE: &str = "a typed buffer view is read as four 32-bit lanes per element: converting from the view's format is the binding's";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedNullNoteReport {
    pub revision: String,
    pub resolved_notes: Vec<ResolvedNote>,
    pub physical_typed_resource_ids: Vec<u32>,
    pub matched_null_typed_resource_ids: Vec<u32>,
    pub shader_values_or_ir_changed: bool,
    pub grants_current_host_authority: bool,
}

/// Internal only: core must have executed sealed56 projection in this same
/// preparation call. Neither Program nor a deserialized report is a public
/// product input. Independently recheck source identity, descriptors and unread
/// membership; validate every physical view on a throwaway resource-pass clone.
/// Failure cannot remove notes or otherwise mutate the source program.
pub(crate) fn resolve_projected_typed_note(
    program: &mut Program,
    contract: &CallerContract,
    cap: &CallerCapability,
    projection: &CallerProjectionReport,
) -> Result<TypedNullNoteReport, String> {
    let mut report = TypedNullNoteReport {
        revision: TYPED_NULL_NOTE_REVISION.into(),
        resolved_notes: vec![],
        physical_typed_resource_ids: vec![],
        matched_null_typed_resource_ids: vec![],
        shader_values_or_ir_changed: false,
        grants_current_host_authority: false,
    };
    if !program.notes.iter().any(|n| n == TYPED_NOTE) {
        return Ok(report);
    }
    let evidence = cap.evidence();
    if program.stage != iridia::Stage::Pixel
        || program.interface.stage != iridia::Stage::Pixel
        || program.provenance.source_hash.as_deref()
            != Some(format!("sha256:{}", evidence.original_dxbc_sha256).as_str())
        || projection.revision != CALLER_REVISION
        || evidence.revision != CALLER_REVISION
        || &projection.evidence != evidence
        || !projection.complete_resource_projection
        || !projection.unchanged_global_declarations
        || !projection.unchanged_control_flow
        || projection.grants_current_host_authority
        || evidence.grants_current_host_authority
    {
        return Err("typed-note resolution requires the same sealed source projection".into());
    }
    let unread = program.unread_resources().map_err(|e| e.to_string())?;
    for r in &program.interface.resources {
        if !matches!(r.kind, ResourceKind::Buffer(BufferKind::Typed(_))) {
            continue;
        }
        let d = r.provenance.d3d.ok_or("typed source register missing")?;
        if d.class != D3dClass::ShaderResource || d.space != 0 || r.count != 1 {
            return Err("unsupported typed source declaration".into());
        }
        let matches: Vec<_> = contract
            .resources
            .iter()
            .filter(|c| {
                c.class == RegisterClass::ShaderResource
                    && c.register == d.register
                    && c.space == d.space
            })
            .collect();
        if matches.len() != 1 {
            return Err("one exact caller row per original typed resource required".into());
        }
        let c = matches[0];
        match &c.resource {
            ResourceContractKind::TypedBuffer { .. } => {
                if evidence
                    .null_buffers
                    .iter()
                    .any(|f| f.original_register == format!("ps:t{}", d.register))
                {
                    return Err(
                        "physical typed view cannot replace a sealed original NULL buffer".into(),
                    );
                }
                report.physical_typed_resource_ids.push(r.id.0);
            }
            ResourceContractKind::UnreadDeclaration {
                original_resource_sha256,
                reason: UnreadReason::MatchedNull,
            } => {
                let digest = source_declaration_sha256(r)?;
                if &digest != original_resource_sha256 || !unread.contains(&r.id) {
                    return Err("typed NULL declaration hash/unread proof failed".into());
                }
                // The only reviewed original NULL typed-load family presently
                // admitted by sealed56 is Waves26's eleven guarded t8 loads.
                if evidence.profile != "original-water0-waves26-isolated-v1"
                    || evidence.original_dxbc_sha256
                        != "c3ac426052a491b82f909e73f22ef04f327c7dec1c7230dd614eff2ff3246f27"
                    || d.register != 8
                    || c.group != 1
                    || c.binding != 40
                    || c.count != 1
                {
                    return Err("no reviewed original typed NULL source rule".into());
                }
                let fixtures: Vec<_> = evidence
                    .null_buffers
                    .iter()
                    .filter(|f| f.original_register == "ps:t8")
                    .collect();
                if fixtures.len() != 1
                    || !fixtures[0].resource_is_null
                    || fixtures[0].format != "R32G32B32A32_FLOAT"
                    || fixtures[0].binding != 40
                {
                    return Err("sealed typed NULL fixture missing".into());
                }
                let evidence_rows: Vec<_> = projection
                    .unread_resources
                    .iter()
                    .filter(|x| x.resource_id == r.id.0)
                    .collect();
                if evidence_rows.len() != 1
                    || evidence_rows[0].register != 8
                    || evidence_rows[0].reason != "MatchedNull"
                    || evidence_rows[0].original_resource_sha256 != digest
                {
                    return Err("typed NULL projection digest/reason mismatch".into());
                }
                let ops: Vec<_> = projection
                    .operations
                    .iter()
                    .filter(|o| o.source_register == 8)
                    .collect();
                let unique: BTreeSet<_> = ops.iter().map(|o| (&o.function, o.expression)).collect();
                if ops.len() != 11
                    || unique.len() != 11
                    || ops.iter().any(|o| {
                        o.source_kind != "guarded-buffer-load"
                            || o.source_result_semantics != "DefinedMatchedNullZero"
                    })
                {
                    return Err("exact eleven original typed NULL load projections required".into());
                }
                report.matched_null_typed_resource_ids.push(r.id.0);
            }
            _ => {
                return Err(
                    "typed resource has neither exact physical view nor matched NULL proof".into(),
                );
            }
        }
    }
    if report.matched_null_typed_resource_ids.is_empty() {
        // The ordinary v2 resource pass remains responsible for physical views.
        return Ok(report);
    }
    let mut checked = program.clone();
    let checked_manifest = apply_resources(&mut checked, contract)?;
    for id in &report.physical_typed_resource_ids {
        if !checked_manifest.resources.iter().any(|r| {
            r.source_resource_id == *id
                && r.physical_binding_required
                && matches!(r.resource, ResourceContractKind::TypedBuffer { .. })
        }) {
            return Err("exact physical typed view validation missing".into());
        }
    }
    let mut ids = report.physical_typed_resource_ids.clone();
    ids.extend(&report.matched_null_typed_resource_ids);
    ids.sort_unstable();
    report.resolved_notes=program.notes.iter().filter(|n|n.as_str()==TYPED_NOTE).map(|n|ResolvedNote{
        original_note:n.clone(),rule:TYPED_NULL_NOTE_REVISION.into(),source_resource_ids:ids.clone(),
        source_and_transport_bits:"All original typed resources checked: physical views use the exact v2 view contract; matched-NULL results were source-projected to same-type zero and declarations are independently entrypoint-unread. No view conversion, format, backing extent or replacement buffer is required for those original NULL resources.".into(),
    }).collect();
    program.notes.retain(|n| n != TYPED_NOTE);
    Ok(report)
}
