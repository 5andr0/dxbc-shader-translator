//! Explicit compute resource and target-limit preparation; host catalogue and
//! actual device/dispatch admission remain with the consumer.
use crate::{CacheIdentity,Error,PreparedShader,Result,SourceShader,Stage,TargetContract};
use crate::resource_contract::{CallerContract,apply_resources,pipeline_bindings};
use crate::prepare::{bound_program,source_declarations};
use serde_json::json;
use std::collections::BTreeMap;
pub const COMPUTE_REVISION:&str="d2r-iridia50-explicit-compute-target-v2";
pub fn prepare_compute(profile:&str,source:&SourceShader,caller:&CallerContract,target:&TargetContract)->Result<PreparedShader> {
    prepare_compute_impl104(profile,source,caller,target,false)
}
/// Opt-in CPU controlled preparation; never original catalogue/device authority.
pub fn prepare_controlled_compute104(profile:&str,source:&SourceShader,caller:&CallerContract,target:&TargetContract)->Result<PreparedShader> {
    prepare_compute_impl104(profile,source,caller,target,true)
}
fn prepare_compute_impl104(profile:&str,source:&SourceShader,caller:&CallerContract,target:&TargetContract,controlled:bool)->Result<PreparedShader> {
    if profile.is_empty()||profile.len()>512||!profile.is_ascii()||source.identity().stage!=Stage::Compute{return Err(Error::Unsupported("compute profile/stage selection".into()))}
    target.validate()?;target.resources(caller)?;
    if caller.resources.iter().any(|r|matches!(r.resource,crate::resource_contract::ResourceContractKind::UnreadDeclaration{..})){return Err(Error::Unsupported("compute has no admitted unread caller projection".into()))}
    let (mut p,sampling)=if controlled {crate::prepare::bound_controlled_program104(source,caller)?}else{bound_program(source,caller)?};
    let declarations=source_declarations(&p)?;let original_interface=serde_json::to_value(&p.interface)?;let original_notes=p.notes.clone();
    let compute=target.compute_module(&p.module)?;
    let bindings=apply_resources(&mut p,caller).map_err(Error::Unsupported)?;
    let device_notes=iridia::compute_limit_notes(&p.module);
    let resolved_device_notes=p.notes.iter().filter(|n|device_notes.contains(n)).cloned().collect::<Vec<_>>();
    // The actual module's size/memory limits were checked above, rather than
    // accepting a note on message shape alone.
    p.notes.retain(|n|!device_notes.contains(n));
    if !p.notes.is_empty(){return Err(Error::Unsupported(format!("unresolved compute notes: {:?}",p.notes)))}
    p.validate().map_err(|e|Error::Validation(format!("prepared compute Iridia: {e:?}")))?;
    let payload=if p.precision.is_precision_bearing() {
        let carrier=crate::precision_carrier::prepare_spirv(&p,&iridia::emit::Pipeline{bindings:pipeline_bindings(&p),..Default::default()}).map_err(Error::Unsupported)?;
        crate::payload104::ShaderPayload104::controlled(&carrier)?
    } else {
        let written=iridia::emit::wgsl_for(&p,&iridia::emit::Pipeline{bindings:pipeline_bindings(&p),..Default::default()}).map_err(|e|Error::Unsupported(e.to_string()))?;
        if !written.notes.is_empty(){return Err(Error::Unsupported(format!("unapproved compute emitter notes: {:?}",written.notes)))}
        crate::payload104::ShaderPayload104::Wgsl{text:written.text.into()}
    };
    let storage_transport=crate::validation::storage_buffer_transport_module(&payload.module()?,caller)?;
    let contract=json!({"profile":profile,"resources":caller,"draw":null,"catalogue_admission_required":true});
    let target_json=serde_json::to_value(target)?;let linkage=serde_json::Value::Null;
    let manifest=json!({"stage":"compute","entry_point":p.entry,"original_interface":original_interface,"prepared_interface":p.interface,"source_declarations":declarations,"bindings":bindings,"draw":null,"sampling":sampling,"compute_contract":compute,"storage_buffer_transport":storage_transport,"original_notes":original_notes,"resolved_device_notes":resolved_device_notes,"outstanding_notes":p.notes,"stock_naga_validated":true,"gpu_verified":false});
    let passes=BTreeMap::from([(COMPUTE_REVISION.to_string(),2),(crate::resource_contract::RESOURCE_REVISION.to_string(),2),(crate::sample_lod::SAMPLE_LOD_REVISION.to_string(),2)]);
    let id=CacheIdentity::current(source.identity().clone(),&contract,&linkage,&target_json,&manifest,passes)?;
    PreparedShader::checked_payload(id,contract,linkage,target_json,manifest,payload)
}
