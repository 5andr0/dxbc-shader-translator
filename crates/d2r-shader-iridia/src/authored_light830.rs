//! New authored receiver admission over a fully revalidated original source proof.
use crate::{PreparedShader,PreparedGraphics,CacheIdentity,Stage,Result,Error};
use crate::receiver_domain830::{Domain,RULE};use crate::resource_contract::{CallerContract,BindingManifest,ResourceContractKind as K,BufferView};
use serde_json::{Value,json};
const PREFIX:&str="authored-idle830:";
fn need(v:bool,s:&str)->Result<()>{if v{Ok(())}else{Err(Error::Identity(format!("830: {s}")))}}
fn allowed(s:&str)->bool{matches!(s,"original-standard630-flag2-live19-668-v1"|"original-background630-active19-three677-v1"|"original-water630-current645-v1"|"actor177-ActorBody104-v1"|"actor177-ActorSkin152-v1"|"actor177-HairOpaque6-v1"|"actor177-HairWeighted7-v1")}
fn shape(base:&PreparedShader,domain:&Domain)->Result<(String,Value,Value)>{
 let original=base.caller_contract()["profile"].as_str().ok_or_else(||Error::Identity("830 source profile".into()))?;need(allowed(original),"exact receiver family; UNIT consumed-count proof cannot widen")?;
 need(base.caller_contract()["authored830"].is_null(),"no recursive authored wrapper")?;
 if !base.manifest()["caller_projection"].is_null(){let r=&base.manifest()["caller_projection"];need(r["evidence"]["unit_consumed_t4"].is_null(),"inactive-point caller proof is not an authored permit")?;for op in r["operations"].as_array().ok_or_else(||Error::Identity("830 original projection operations".into()))?{need(op["source_result_semantics"]!="UnobservedUnderConsumedCount","new points may not activate an old consumed-count exclusion")?;}}
 let mut contract=base.caller_contract().clone();let mut resources:CallerContract=base.resource_contract()?;let mut n=0;
 for r in &mut resources.resources{if r.register==8{if let K::TypedBuffer{format,view}=&mut r.resource{need(base.stage()==Stage::Fragment&&format=="R32G32B32A32_FLOAT"&&view.source_first_element==0&&view.source_stride_bytes==16&&matches!((view.source_num_elements,view.source_resource_bytes),(19,304)|(512,8192)),"exact original live t8 dynamic float4 view")?;*view=BufferView{source_first_element:0,source_num_elements:31,source_stride_bytes:16,source_resource_bytes:496};n+=1;}}}
 need(n==usize::from(base.stage()==Stage::Fragment),"receiver stage has exact t8 declaration")?;let profile=format!("{PREFIX}{original}");contract["profile"]=json!(profile);contract["resources"]=serde_json::to_value(&resources)?;
 for key in ["standard668","background677","water649"]{contract.as_object_mut().unwrap().remove(key);}
 contract["authored830"]=json!({"schema":"AuthoredReceiverStage830-v1","domain":domain.receipt(),"base_original_artifact":base,"source_projection_scope":"matched NULL results unchanged; Standard/background unchanged bank shadow/volume guards,ordinary0/spot0/unit1 and unchanged unit sentinel at new27; point loop bounded by exact31 domain","runtime_authority":false});
 let mut manifest=base.manifest().clone();let mut bindings:BindingManifest=base.binding_layout()?;for entry in &mut bindings.resources{if let Some(r)=resources.resources.iter().find(|r|r.class==entry.class&&r.register==entry.register){entry.resource=r.resource.clone();}}
 manifest["bindings"]=serde_json::to_value(bindings)?;Ok((profile,contract,manifest))
}
fn stage(base:&PreparedShader,domain:&Domain)->Result<PreparedShader>{
 base.validate_artifact()?;let(_,caller,manifest)=shape(base,domain)?;let mut passes=base.identity().passes.clone();passes.insert(RULE.into(),1);let id=CacheIdentity::current(base.identity().source.clone(),&caller,base.linked_interface(),base.target_capabilities(),&manifest,passes)?;
 PreparedShader::checked_payload(id,caller,base.linked_interface().clone(),base.target_capabilities().clone(),manifest,base.payload().clone())
}
pub fn prepare(base:&PreparedGraphics,domain:&Domain)->Result<PreparedGraphics>{base.validate_artifact()?;let out=PreparedGraphics{schema:base.schema.clone(),profile:format!("{PREFIX}{}",base.profile),requires_host_profile_admission:true,vertex:stage(&base.vertex,domain)?,fragment:stage(&base.fragment,domain)?,linkage:base.linkage.clone()};out.validate_artifact()?;need(out.vertex.payload()==base.vertex.payload()&&out.fragment.payload()==base.fragment.payload(),"actual authored payload must stay bit-identical")?;Ok(out)}
/// Revalidate cached authored context first, then the unchanged generic module,
/// declaration/bounds/precision/projection checks still run on the new31 stage.
pub(crate)fn validate_stage(p:&PreparedShader)->Result<Option<String>>{
 let profile=p.caller_contract()["profile"].as_str().unwrap_or("");let marker=&p.caller_contract()["authored830"];
 if !profile.starts_with(PREFIX){need(marker.is_null(),"authored marker on an ordinary profile")?;return Ok(None)}
 need(marker["schema"]=="AuthoredReceiverStage830-v1"&&marker["runtime_authority"]==false,"authored source marker")?;
 let base:PreparedShader=serde_json::from_value(marker["base_original_artifact"].clone())?;need(base.caller_contract()["authored830"].is_null()&&!base.caller_contract()["profile"].as_str().unwrap_or("").starts_with(PREFIX),"no recursive authored source proof")?;base.validate_artifact()?;
 let domain=Domain::from_receipt(&marker["domain"]).map_err(Error::Identity)?;let(expected,caller,manifest)=shape(&base,&domain)?;let mut passes=base.identity().passes.clone();passes.insert(RULE.into(),1);
 need(profile==expected&&*p.caller_contract()==caller&&*p.manifest()==manifest&&p.identity().source==base.identity().source&&p.identity().passes==passes&&p.payload()==base.payload()&&p.linked_interface()==base.linked_interface()&&p.target_capabilities()==base.target_capabilities(),"whole new authored profile/domain/base/payload/layout/source/projection join")?;
 Ok(Some(base.caller_contract()["profile"].as_str().unwrap().into()))
}

pub fn original_pair(pair:&PreparedGraphics)->Result<PreparedGraphics>{pair.validate_artifact()?;let v:PreparedShader=serde_json::from_value(pair.vertex.caller_contract()["authored830"]["base_original_artifact"].clone())?;let f:PreparedShader=serde_json::from_value(pair.fragment.caller_contract()["authored830"]["base_original_artifact"].clone())?;let out=PreparedGraphics{schema:pair.schema.clone(),profile:v.caller_contract()["profile"].as_str().ok_or_else(||Error::Identity("830 original paired profile".into()))?.into(),requires_host_profile_admission:true,vertex:v,fragment:f,linkage:pair.linkage.clone()};out.validate_artifact()?;Ok(out)}
