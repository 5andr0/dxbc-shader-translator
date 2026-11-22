//! Bounded memory-form selections. Never lowers or speculates source control flow.
//! The writer/parser reports come from the same executions as the precision join.
use iridia_naga as ir;
use ir::correspondence::{EmissionReport, EmittedFlow, ExpressionRef as SourceRef, Owner as SourceOwner};
use naga::correspondence::{ImportReport, ImportedArithmetic, ImportedFlow, Owner};

const REFUSAL: &str = "PrecisionImportUnsupportedPhiOrStructuredFlow";
fn fail(detail: &str) -> String { format!("{REFUSAL}: {detail}") }

/// Leave straight-line admission unchanged. Bound the extension to a single
/// entry with ordinary memory stores and properly nested if/else selections.
/// Loops, switches, calls, early exits and effects need a separate proof.
pub(super) fn preflight(m: &ir::Module) -> Result<bool, String> {
    fn scan(b: &ir::Block) -> Result<bool, String> {
        let mut found = false;
        for s in b.iter() {
            match s {
                ir::Statement::If { accept, reject, .. } => { found = true; scan(accept)?; scan(reject)?; }
                ir::Statement::Block(b) => { found |= scan(b)?; }
                ir::Statement::Loop { .. } | ir::Statement::Switch { .. } => return Err(fail("loop/switch")),
                _ => {}
            }
        }
        Ok(found)
    }
    let mut found = false;
    for (_, f) in m.functions.iter() { found |= scan(&f.body)?; }
    for e in &m.entry_points { found |= scan(&e.function.body)?; }
    if !found { return Ok(false); }
    if !m.functions.is_empty() || m.entry_points.len() != 1 { return Err(fail("selection owner lifecycle")); }
    fn shape(b: &ir::Block, nested: bool) -> Result<(), String> {
        for s in b.iter() {
            match s {
                ir::Statement::Emit(_) | ir::Statement::Store { .. } => {},
                ir::Statement::Block(b) => shape(b, nested)?,
                ir::Statement::If { accept, reject, .. } => {
                    if accept.is_empty() && reject.is_empty() { return Err(fail("elided empty selection")); }
                    shape(accept, true)?; shape(reject, true)?;
                }
                ir::Statement::Return { .. } if !nested => {},
                _ => return Err(fail("selection effect/early exit")),
            }
        }
        Ok(())
    }
    shape(&m.entry_points[0].function.body, false)?;
    Ok(true)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event { If(u32), Else, End, Arithmetic(u32), Store(u32, u32), Wrapped(u32, u32, [u32; 2]) }
#[derive(Clone, Debug)]
pub(super) struct SelectionProof584 {
    pub selections: usize,
    pub branch_stores: usize,
    pub arithmetic: usize,
    /// All events, including branch polarity, statement order and merge points.
    trace: Vec<Event>,
}
impl SelectionProof584 {
    pub(super) fn evidence(&self) -> String {
        format!("selection584-v1:{}:{}:{}:{:?}", self.selections, self.branch_stores, self.arithmetic, self.trace)
    }
}

pub(super) fn join(
    source: &ir::Module, imported: &naga::Module, emitted: &EmissionReport, report: &ImportReport,
) -> Result<SelectionProof584, String> {
    if emitted.flow_revision != 1 || report.flow_revision != 1 { return Err(fail("flow report revision")); }
    if !report.flow_adapted_functions.is_empty() { return Err(fail("phi/spill")); }
    let mut source_trace = vec![];
    fn src(b: &ir::Block, owner: SourceOwner, nested: bool, e: &EmissionReport, trace: &mut Vec<Event>) -> Result<(), String> {
        let r = |expression| SourceRef { owner, expression };
        for s in b.iter() {
            match s {
                ir::Statement::Emit(range) => for h in range.clone() {
                    let rows: Vec<_> = e.arithmetic.iter().filter(|a| a.producer == r(h)).collect();
                    if rows.len() > 1 { return Err(fail("ambiguous arithmetic emission")); }
                    if let Some(row) = rows.first() { trace.push(Event::Arithmetic(row.result_id)); }
                    let calls: Vec<_> = e.flow.iter().filter_map(|a| match a {
                        EmittedFlow::WrappedBinary { producer, callee_id, result_id, arguments } if *producer == r(h) => Some(Event::Wrapped(*callee_id, *result_id, *arguments)), _ => None,
                    }).collect();
                    if calls.len() > 1 { return Err(fail("ambiguous wrapped emission")); }
                    trace.extend(calls);
                },
                ir::Statement::Store { pointer, value } if nested => {
                    let rows: Vec<_> = e.flow.iter().filter_map(|a| match a {
                        EmittedFlow::Store { pointer: p, value: v, pointer_id, value_id } if *p == r(*pointer) && *v == r(*value) => Some((*pointer_id, *value_id)),
                        _ => None,
                    }).collect();
                    if rows.len() != 1 { return Err(fail("branch store emission not direct/unique")); }
                    trace.push(Event::Store(rows[0].0, rows[0].1));
                },
                ir::Statement::If { condition, accept, reject } => {
                    let rows: Vec<_> = e.flow.iter().filter_map(|a| match a {
                        EmittedFlow::Selection { condition: c, condition_id } if *c == r(*condition) => Some(*condition_id), _ => None,
                    }).collect();
                    if rows.len() != 1 { return Err(fail("selection emission not unique")); }
                    trace.push(Event::If(rows[0])); src(accept, owner, true, e, trace)?;
                    trace.push(Event::Else); src(reject, owner, true, e, trace)?; trace.push(Event::End);
                },
                ir::Statement::Block(b) => src(b, owner, nested, e, trace)?,
                ir::Statement::Store { .. } | ir::Statement::Return { .. } if !nested => {},
                _ => return Err(fail("source trace shape")),
            }
        }
        Ok(())
    }
    src(&source.entry_points[0].function.body, SourceOwner::EntryPoint(0), false, emitted, &mut source_trace)?;
    let conditions: Vec<_> = source_trace.iter().filter_map(|e| if let Event::If(id) = e { Some(*id) } else { None }).collect();
    let mut owners = vec![];
    for id in &conditions {
        let rows: Vec<_> = report.flow.iter().filter_map(|e| match e {
            ImportedFlow::Selection { condition_id, condition, direct: true, .. } if condition_id == id => Some(condition.owner), _ => None,
        }).collect();
        if rows.len() != 1 { return Err(fail("selection import not direct/unique")); }
        if !owners.contains(&rows[0]) { owners.push(rows[0]); }
    }
    if owners.len() != 1 { return Err(fail("selection import owner")); }
    let owner = owners[0];
    let Owner::Function(h) = owner else { return Err(fail("selection import wrapper")); };
    // Existing writer-generated binary wrappers are part of the consumed
    // payload. Every selected-body call must join their actual IDs below.
    for (other, helper) in imported.functions.iter().filter(|(other, _)| *other != h) {
        if !report.flow.iter().any(|e| matches!(e, ImportedFlow::BinaryCall { result, callee, direct: true, .. } if result.owner == owner && *callee == other))
            || !helper.local_variables.is_empty()
            || !helper.expression_controls.is_empty()
            || helper.body.iter().any(|s| !matches!(s, naga::Statement::Emit(_) | naga::Statement::Return { value: Some(_) })) {
            return Err(fail("unjoined/effectful helper lifecycle"));
        }
    }
    let f = &imported.functions[h];
    let mut imported_trace = vec![];
    fn dst(b: &naga::Block, owner: Owner, nested: bool, report: &ImportReport, trace: &mut Vec<Event>) -> Result<(), String> {
        let r = |expression| naga::correspondence::ExpressionRef { owner, expression };
        for s in b.iter() {
            match s {
                naga::Statement::Emit(range) => for h in range.clone() {
                    let rows: Vec<_> = report.arithmetic.iter().filter_map(|a| match a {
                        ImportedArithmetic::Direct { producer, result_id, .. } if *producer == r(h) => Some(*result_id), _ => None,
                    }).collect();
                    if rows.len() > 1 { return Err(fail("ambiguous arithmetic import")); }
                    if let Some(id) = rows.first() { trace.push(Event::Arithmetic(*id)); }
                },
                naga::Statement::Store { pointer, value } if nested => {
                    let rows: Vec<_> = report.flow.iter().filter_map(|a| match a {
                        ImportedFlow::Store { pointer: p, value: v, pointer_id, value_id, direct: true, .. } if *p == r(*pointer) && *v == r(*value) => Some((*pointer_id, *value_id)), _ => None,
                    }).collect();
                    if rows.len() != 1 { return Err(fail("branch store import not direct/unique")); }
                    trace.push(Event::Store(rows[0].0, rows[0].1));
                },
                naga::Statement::If { condition, accept, reject } => {
                    let rows: Vec<_> = report.flow.iter().filter_map(|a| match a {
                        ImportedFlow::Selection { condition: c, condition_id, direct: true, .. } if *c == r(*condition) => Some(*condition_id), _ => None,
                    }).collect();
                    if rows.len() != 1 { return Err(fail("final selection import not direct/unique")); }
                    trace.push(Event::If(rows[0])); dst(accept, owner, true, report, trace)?;
                    trace.push(Event::Else); dst(reject, owner, true, report, trace)?; trace.push(Event::End);
                },
                naga::Statement::Call { function, arguments, result: Some(result) } => {
                    let rows: Vec<_> = report.flow.iter().filter_map(|e| match e {
                        ImportedFlow::BinaryCall { callee, callee_id, result: p, result_id, arguments: args, argument_ids, direct: true, .. }
                            if *callee == *function && *p == r(*result) && arguments.len() == 2
                                && args[0] == r(arguments[0]) && args[1] == r(arguments[1]) => Some(Event::Wrapped(*callee_id, *result_id, *argument_ids)),
                        _ => None,
                    }).collect();
                    if rows.len() != 1 { return Err(fail("wrapped import not direct/unique")); }
                    trace.extend(rows);
                },
                naga::Statement::Block(b) => dst(b, owner, nested, report, trace)?,
                naga::Statement::Store { .. } | naga::Statement::Return { .. } if !nested => {},
                _ => return Err(fail("imported trace shape")),
            }
        }
        Ok(())
    }
    dst(&f.body, owner, false, report, &mut imported_trace)?;
    // No dropped, duplicated, adapted or newly-created arithmetic is admitted.
    let n = source_trace.iter().filter(|e| matches!(e, Event::Arithmetic(_))).count();
    let selections = source_trace.iter().filter(|e| matches!(e, Event::If(_))).count();
    if emitted.arithmetic.len() != n || report.arithmetic.len() != n
        || report.arithmetic.iter().any(|e| matches!(e, ImportedArithmetic::Unsupported { .. }))
        || emitted.flow.iter().filter(|e| matches!(e, EmittedFlow::Selection { .. })).count() != selections
        || report.flow.iter().filter(|e| matches!(e, ImportedFlow::Selection { .. })).count() != selections {
        return Err(fail("flow inventory coverage"));
    }
    if source_trace != imported_trace { return Err(fail("condition/polarity/ordered producer or branch store mismatch")); }
    Ok(SelectionProof584 { selections, branch_stores: source_trace.iter().filter(|e| matches!(e, Event::Store(..))).count(), arithmetic: n, trace: source_trace })
}
