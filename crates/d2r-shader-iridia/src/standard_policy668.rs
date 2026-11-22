//! Authored adapter policy. Original and host inputs are supplied at run time and
//! authenticated; none are stored in the repository.
//! One exact original Standard pair under immutable shared630 flag2/live19 inputs.
//! No background/UNIT/Water permit, resource substitution or live GPU authority.
use super::{CallerCapability,CallerEvidence,CallerProjectionReport,NullImageFixture,ProjectedOperation,UnreadResource,CALLER_REVISION};
use crate::{Error,Result,SourceIdentity,SourceShader,Stage,PreparedShader};
use crate::resource_contract::{CallerContract,RegisterClass as C,ResourceContractKind as K,UnreadReason,RESOURCE_REVISION};
use serde_json::Value;
use std::{collections::{BTreeMap,BTreeSet},sync::Arc};
use iridia_naga as ir;
use ir::Expression as E;
#[path="standard668_cpu.rs"]mod cpu;
pub use cpu::{PROFILE,RULE};
pub(crate) use cpu::{NULLS,PHYSICAL};
fn need(b:bool,s:&str)->Result<()>{if b{Ok(())}else{Err(Error::Unsupported(format!("Standard668: {s}")))}}
fn source(s:&SourceIdentity)->Result<()>{cpu::source(match s.stage{Stage::Vertex=>0,Stage::Fragment=>1,_=>2},&s.path,&s.resource_sha256,s.resource_bytes,s.offset,s.byte_count,&s.dxbc_sha256).map_err(Error::Identity)}

fn fixtures()->Vec<NullImageFixture>{cpu::NULLS.into_iter().map(|s|NullImageFixture{
    profile:PROFILE.into(),original_dxbc_sha256:cpu::PS.into(),group:1,binding:32+s,original_register:format!("ps:t{s}"),
    api:"D3D12-descriptor-table-null-SRV".into(),resource_is_null:true,dimension:match s{33=>"TextureCube",39|40=>"Texture2DArray",45..=48=>"Texture3D",_=>"Texture2D"}.into(),
    format:"R32G32B32A32_FLOAT".into(),array_layers:if matches!(s,39|40){1}else{0},mip_levels:1,first_array_slice:0,most_detailed_mip:0,
    shader_component_mapping:5768,plane_slice:0,resource_min_lod_clamp_bits:0,accepted_host_provenance_sha256:cpu::CONTRACT.into(),rule_revision:RULE.into(),
}).collect()}
fn evidence(e:&CallerEvidence)->Result<()>{need(e.revision==CALLER_REVISION&&e.profile==PROFILE&&e.original_dxbc_sha256==cpu::PS&&e.fixtures==fixtures()&&
    e.unit_consumed_t4.is_none()&&e.null_buffers.is_empty()&&!e.grants_current_host_authority,"exact Standard630 source-null descriptors; no old capability")}
/// The Standard668 source types have no values in this program.
pub enum StandardSource668 {}
pub enum StandardCapability668 {}
impl StandardCapability668 {
    pub(crate) fn seal(owner: &Arc<StandardSource668>, _: usize, _: &SourceShader, _: &SourceShader) -> Result<Self> { match **owner {} }
    pub(crate) fn projection(&self) -> &CallerCapability { match *self {} }
    pub(crate) fn recheck(&self, _: &Arc<StandardSource668>, _: &SourceShader, _: &SourceShader) -> Result<()> { match *self {} }
    pub(crate) fn receipt(&self) -> Result<Value> { match *self {} }
}
fn receipt(_: usize) -> Result<Value> {
    Err(Error::Unsupported("Standard668: not part of this build".into()))
}
/// Validate the unchanged physical recipe before projection; afterwards only
/// proved source-null resources and their dead sampler dependencies may omit binds.
pub(crate)fn contract(stage:Stage,c:&CallerContract,projected:bool)->Result<()>{
    let group=match stage{Stage::Vertex=>0,Stage::Fragment=>1,_=>return Err(Error::Identity("Standard668 stage".into()))};
    let cb:&[u32]=if group==0{&[0,3]}else{&[0,1,2,3,4,6,10]};
    let sam:&[u32]=if group==0{&[]}else{&[0,1,3,4,6,7]};
    need(c.revision==RESOURCE_REVISION&&c.resources.len()==if group==0{2}else{34},"complete original caller/revision")?;
    let mut keys=BTreeSet::new();
    for r in &c.resources{
        let s=r.register;need(r.group==group&&r.space==0&&r.count==1&&keys.insert((r.class,s)),"whole original caller namespace/uniqueness")?;
        let b=match r.class{C::ConstantBuffer=>Some(s),C::ShaderResource=>s.checked_add(32),C::Sampler=>s.checked_add(96),_=>None}.ok_or_else(||Error::Identity("Standard668 namespace".into()))?;
        let null=group==1&&r.class==C::ShaderResource&&cpu::NULLS.contains(&s);
        let dead_sampler=group==1&&r.class==C::Sampler&&[3,4,6,7].contains(&s);
        let valid=if projected&&(null||dead_sampler){
            matches!(&r.resource,K::UnreadDeclaration{reason,..}if *reason==if dead_sampler{UnreadReason::UnusedProjectionDependency}else if matches!(s,41|44){UnreadReason::MatchedNull}else{UnreadReason::UnobservedSourceBranch})
        }else{match &r.resource{
            K::ConstantBuffer{bound_bytes}=>r.class==C::ConstantBuffer&&cb.contains(&s)&&*bound_bytes==1536,
            K::StructuredBuffer{stride_bytes,view}=>group==1&&r.class==C::ShaderResource&&matches!(s,4|5)&&*stride_bytes==if s==4{4}else{1024}&&view.source_first_element==0&&view.source_num_elements==1&&view.source_stride_bytes==u64::from(*stride_bytes)&&view.source_resource_bytes==u64::from(*stride_bytes),
            K::TypedBuffer{format,view}=>group==1&&r.class==C::ShaderResource&&s==8&&format=="R32G32B32A32_FLOAT"&&view.source_first_element==0&&view.source_num_elements==19&&view.source_stride_bytes==16&&view.source_resource_bytes==304,
            K::Texture{dimension,arrayed,multisampled,sample}=>group==1&&r.class==C::ShaderResource&&(null||cpu::PHYSICAL.contains(&s))&&
                dimension==match s{33|34=>"cube",45..=48=>"d3",_=>"d2"}&&*arrayed==matches!(s,39|40)&&!*multisampled&&sample=="float",
            K::Sampler{comparison}=>r.class==C::Sampler&&sam.contains(&s)&&*comparison==(s>=6),_=>false,
        }};
        need(r.binding==b&&valid,"original physical caller changed; no substituted view/light/texture")?;
    }Ok(())
}
pub(crate)fn apply(p:&mut iridia::Program,cap:&CallerCapability)->Result<CallerProjectionReport>{
    evidence(cap.evidence())?;
    project(p,cap.evidence(),Shape::Refuse)
}
/// What the projection does when the lifted module identity differs from `cpu::SHAPE`.
/// This authority always refuses; only the separately sealed map authority may warn.
#[derive(Clone,Copy,PartialEq,Eq)]
pub(crate)enum Shape{Refuse,Warn(&'static str)}
/// The Standard630 source-null projection itself (shared by this authority and the separately
/// sealed map authority `standard_map1129`); the caller has checked its own evidence.
pub(crate)fn project(p:&mut iridia::Program,evidence:&CallerEvidence,shape:Shape)->Result<CallerProjectionReport>{
    need(p.stage==iridia::Stage::Pixel&&p.provenance.source_hash.as_deref()==Some(format!("sha256:{}",cpu::PS).as_str()),"original Standard PS provenance")?;
    p.precision.validate(&p.module,p.provenance.source_hash.as_deref()).map_err(Error::Unsupported)?;
    // Exact immutable631 bound lift: seals every store, bit extraction, sampler,
    // branch, loop entry/break, guard Emit and nonnull arithmetic/declaration.
    // If a consumed compiler/lifter changes this shape, refuse and obtain new
    // immutable source proof; never drop this check to make preparation succeed.
    let lifted=iridia::precision::module_identity(&p.module);
    if lifted!=cpu::SHAPE{if let Shape::Warn(who)=shape{eprintln!("{who}: WARNING lifted Standard PS module identity {lifted} differs from the Standard668 shape pin {}; continuing under the map authority (every other check unchanged)",cpu::SHAPE);}}
    need((lifted==cpu::SHAPE||shape!=Shape::Refuse)&&p.module.functions.is_empty()&&p.module.entry_points.len()==1&&p.interface.resources.len()==34,"exact original bound source execution/declaration shape")?;
    let mut globals=BTreeMap::new();let mut declarations=BTreeMap::new();
    for r in &p.interface.resources{if let Some(d)=r.provenance.d3d{if d.class==iridia::D3dClass::ShaderResource&&cpu::NULLS.contains(&d.register){
        need(d.space==0&&r.count==1,"original null declaration namespace")?;
        let g=p.global(r.id).ok_or_else(||Error::Identity("Standard668 source global".into()))?;
        super::validate_image_declaration(p,g,d.register,PROFILE).map_err(Error::Unsupported)?;
        globals.insert(g,d.register);declarations.insert(d.register,(r.id,crate::resource_contract::source_declaration_sha256(r).map_err(Error::Unsupported)?));
    }}}
    need(declarations.len()==12,"complete original null image declarations")?;
    let f=&p.module.entry_points[0].function;let mut types=ir::front::Typifier::new();let ctx=ir::proc::ResolveContext::with_locals(&p.module,&f.local_variables,&f.arguments);
    let mut next=p.clone();let mut operations=vec![];let mut counts=BTreeMap::<u32,[usize;3]>::new();let mut zeros=BTreeSet::new();
    for(h,e)in f.expressions.iter(){
        let E::ImageSample{image,level,depth_ref,gather,..}=e else{continue};let E::GlobalVariable(g)=f.expressions[*image]else{continue};let Some(&slot)=globals.get(&g)else{continue};
        need(gather.is_none(),"unreviewed original NULL gather")?;
        let(kind,k)=match level{ir::SampleLevel::Gradient{..}if depth_ref.is_none()=>("gradient",2),ir::SampleLevel::Exact(_)if depth_ref.is_none()=>("exact",0),ir::SampleLevel::Zero if depth_ref.is_some()=>("comparison-zero",1),_=>return Err(Error::Unsupported("Standard668 exact source operation family".into()))};
        let semantics=cpu::operation(slot,kind).map_err(Error::Unsupported)?;counts.entry(slot).or_default()[k]+=1;
        types.grow(h,&f.expressions,&ctx).map_err(|e|Error::Unsupported(e.to_string()))?;let inner=types.get(h,&p.module.types);
        let ty=next.module.types.insert(ir::Type{name:None,inner:inner.clone()},ir::Span::UNDEFINED);
        next.module.entry_points[0].function.expressions[h]=E::ZeroValue(ty);zeros.insert(h.index());
        operations.push(ProjectedOperation{function:p.module.entry_points[0].name.clone(),expression:h.index(),source_register:slot,source_kind:kind.into(),result_type:format!("{inner:?}"),source_result_semantics:semantics.into()});
    }
    for slot in cpu::NULLS{need(counts.get(&slot).copied()==cpu::operation_counts(slot),"exact source NULL operation counts")?;}
    super::constant_scopes(&mut next.module.entry_points[0].function.body,&zeros);
    next.precision=p.precision.unchanged_arithmetic(&p.module,&next.module,iridia::precision::TransformKind::CallerNull).map_err(Error::Unsupported)?;
    next.validate().map_err(|e|Error::Validation(format!("Standard668 projected source: {e:?}")))?;
    let unread=next.unread_resources().map_err(|e|Error::Validation(e.to_string()))?;
    let mut resources=vec![];
    for(slot,(id,sha))in declarations{need(unread.contains(&id),"source null declaration remains read; no omission")?;
        resources.push(UnreadResource{resource_id:id.0,register:slot,original_resource_sha256:sha,reason:if matches!(slot,41|44){"MatchedNull"}else{"UnobservedSourceBranch"}.into()});}
    *p=next;Ok(CallerProjectionReport{revision:CALLER_REVISION.into(),evidence:evidence.clone(),operations,unread_resources:resources,dead_guard_emits:vec![],unchanged_control_flow:true,unchanged_global_declarations:true,complete_resource_projection:true,grants_current_host_authority:false})
}
pub(crate)fn cached(p:&PreparedShader)->Result<()>{
    if p.caller_contract()["profile"]!=PROFILE{return need(p.caller_contract()["standard668"].is_null(),"Standard receipt on another profile");}
    source(&p.identity().source)?;
    let index=p.caller_contract()["standard668"]["draw_index"].as_u64().filter(|n|*n<417).ok_or_else(||Error::Identity("Standard668 cached draw".into()))?as usize;
    need(p.caller_contract()["standard668"]==receipt(index)?&&p.identity().passes.get(RULE)==Some(&1)&&p.payload().kind()=="controlled-spirv104"&&p.payload().source_sha256()==Some(p.identity().source.dxbc_sha256.as_str())&&p.manifest()["draw"].is_null(),"controlled source/provenance/caller cache identity")?;
    contract(p.stage(),&p.resource_contract()?,p.stage()==Stage::Fragment)?;
    if p.stage()==Stage::Fragment{
        let report:CallerProjectionReport=serde_json::from_value(p.manifest()["caller_projection"].clone())?;evidence(&report.evidence)?;
        let mut counts=BTreeMap::<u32,[usize;3]>::new();let mut seen=BTreeSet::new();
        for op in &report.operations{let semantics=cpu::operation(op.source_register,&op.source_kind).map_err(Error::Identity)?;
            need(op.source_result_semantics==semantics&&seen.insert((&op.function,op.expression)),"cached source operation/semantics coverage")?;
            let k=match op.source_kind.as_str(){"exact"=>0,"comparison-zero"=>1,"gradient"=>2,_=>return Err(Error::Identity("Standard668 operation".into()))};counts.entry(op.source_register).or_default()[k]+=1;}
        for slot in cpu::NULLS{need(counts.get(&slot).copied()==cpu::operation_counts(slot),"cached exact source operation counts")?;}
        need(report.unread_resources.len()==12&&report.unread_resources.iter().all(|r|cpu::NULLS.contains(&r.register)&&r.reason==if matches!(r.register,41|44){"MatchedNull"}else{"UnobservedSourceBranch"}),"cached source-null declaration coverage/reasons")?;
    }Ok(())
}

