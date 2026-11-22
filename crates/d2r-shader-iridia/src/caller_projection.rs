//! Explicit original caller-result projection. No placeholder resources or live-world permits.
//! UNIT's array reads are not read under its consumed count, never zero reads of a
//! mismatched NULL view.
use iridia::{D3dClass, Program};
use iridia_naga as ir;
use ir::{Expression as E, Function, Handle, SampleLevel, TypeInner};
use serde::{Deserialize, Serialize};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};
#[path="caller_guard_liveness.rs"]mod caller_guard_liveness;

pub const CALLER_REVISION:&str="d2r-iridia-explicit-original-caller-results56-v3-dead-guard-emits";
const ROCK:&str="7a396f77cfddbd13aec8f29a33bc1a3262a557fca37dcb8ada0fa97458b5fd1d";
const WATER:&str="c3ac426052a491b82f909e73f22ef04f327c7dec1c7230dd614eff2ff3246f27";
const UNIT:&str="standard-rock0-sourcebank45-strict-owned-unit-v1";

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NullImageFixture {
    pub profile:String,pub original_dxbc_sha256:String,pub group:u32,pub binding:u32,
    pub original_register:String,pub api:String,pub resource_is_null:bool,
    pub dimension:String,pub format:String,pub array_layers:u32,pub mip_levels:u32,
    pub first_array_slice:u32,pub most_detailed_mip:u32,pub shader_component_mapping:u32,
    pub plane_slice:u32,pub resource_min_lod_clamp_bits:u32,
    pub accepted_host_provenance_sha256:String,pub rule_revision:String,
}
/// Borrow from the existing non-deserializable UnitWitness upload bytes.
/// This narrow compile-time gate is not a substitute for that witness's full
/// lifetime checks at submission.
pub struct UnitConsumedBytes<'a>{pub t4:&'a [u8]}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct NullBufferFixture {
    pub profile:String,pub original_dxbc_sha256:String,pub group:u32,pub binding:u32,pub original_register:String,
    pub api:String,pub resource_is_null:bool,pub format:String,pub first_element:u64,pub num_elements:u32,
    pub structure_byte_stride:u32,pub flags:u32,pub shader_component_mapping:u32,pub host_provenance_sha256:String,pub revision:String,
}
impl NullBufferFixture {
    fn validate(&self)->Result<u32,String>{
        let slot=self.binding.checked_sub(32).ok_or("buffer namespace")?;
        let (fmt,stride)=match slot{4=>("UNKNOWN",4),5=>("UNKNOWN",1024),8=>("R32G32B32A32_FLOAT",0),_=>return Err("unknown NULL buffer slot".into())};
        if self.profile!="original-water0-waves26-isolated-v1"||self.original_dxbc_sha256!=WATER||self.group!=1||self.original_register!=format!("ps:t{slot}")||self.api!="D3D12-descriptor-table-null-SRV"||!self.resource_is_null||self.format!=fmt||self.first_element!=0||self.num_elements!=1||self.structure_byte_stride!=stride||self.flags!=0||self.shader_component_mapping!=5768||self.host_provenance_sha256!="b58166b9751741745c12695d8f5b4e32099239799d63532e406b52ccf4efac1a"||self.revision!="waves26-strict-null-images-and-buffers-v1" {return Err("exact Waves NULL buffer descriptor/provenance required".into())}Ok(slot)
    }
}

#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CallerEvidence {
    pub revision:String,pub profile:String,pub original_dxbc_sha256:String,
    pub fixtures:Vec<NullImageFixture>,pub unit_consumed_t4:Option<[u8;4]>,
    pub null_buffers:Vec<NullBufferFixture>,
    pub grants_current_host_authority:bool,
}
/// Sealed by source/descriptor checks; cannot be deserialized from cache evidence.
pub struct CallerCapability {evidence:CallerEvidence,water649:Option<water_policy649::WaterSource649>}
impl CallerCapability {
    pub fn seal(profile:&str,fixtures:Vec<NullImageFixture>,unit:Option<UnitConsumedBytes<'_>>)->Result<Self,String>{
        if profile==water_policy649::PROFILE{return Err("Water649 requires seal_water649 over actual shared630 bytes".into());}
        if profile==foliage_policy883::PROFILE{return Err("Foliage23 requires Foliage23Capability::seal over the source pair and consumed host t4 bytes".into());}
        let (sha,slots)=policy(profile)?;
        let mut fixtures=fixtures;fixtures.sort_by_key(|f|f.binding);
        let actual:Vec<_>=fixtures.iter().map(|f|f.binding.checked_sub(32).ok_or("invalid source slot")).collect::<Result<_,_>>()?;
        if actual!=slots{return Err("incomplete/duplicate original NULL descriptor set".into())}
        for f in &fixtures {validate_fixture(profile,sha,f)?;}
        let unit_consumed_t4=if profile==UNIT {
            let t4=unit.ok_or("UNIT requires borrowed actual consumed t4 bytes, not Summary")?.t4;
            if t4!=[0,0,0,1]{return Err("UNIT exact high-byte unit and low-six zero count required".into())}
            Some(t4.try_into().map_err(|_|"UNIT t4 extent")?)
        }else{if unit.is_some(){return Err("UNIT authority cannot widen another caller".into())}None};
        Ok(Self{evidence:CallerEvidence{revision:CALLER_REVISION.into(),profile:profile.into(),original_dxbc_sha256:sha.into(),fixtures,unit_consumed_t4,null_buffers:vec![],grants_current_host_authority:false},water649:None})
    }
    pub fn seal_waves(images:Vec<NullImageFixture>,buffers:Vec<NullBufferFixture>)->Result<Self,String>{
        let mut cap=Self::seal("original-water0-waves26-isolated-v1",images,None)?;
        let slots=buffers.iter().map(NullBufferFixture::validate).collect::<Result<Vec<_>,_>>()?;
        if slots!=[4,5,8]{return Err("complete canonical Waves NULL buffer set required".into())}
        cap.evidence.null_buffers=buffers;Ok(cap)
    }
    pub fn evidence(&self)->&CallerEvidence{&self.evidence}
}
fn policy(p:&str)->Result<(&'static str,Vec<u32>),String>{
    let actor=match p {
        "actor177-HairOpaque6-v1"=>Some("79a13e5e3a60959701dbbf430a75029754b9fc9ce8b3801af172cc6c008fbee3"),
        "actor177-HairWeighted7-v1"=>Some("7c2cd8ea2c470bfc2f14aa8713bcec9fa7940fea29c6aaa78c3f70f33f4dbeae"),
        "actor177-ActorSkin152-v1"=>Some("e961dcf8b85b6536eb9375d3a71c77ee0b709c42f9ae541f7ff2cf6d8e4521d6"),
        "actor177-ActorBody104-v1"=>Some("d63ba46f230de4f4e31f8e81a0325eec2f9b8a4e1c4239561db5ba9b95a57c4b"),_=>None};
    if let Some(sha)=actor {let mut s=vec![33,39,40,44,45,46,47,48];if p.contains("ActorSkin")||p.contains("ActorBody"){s.extend([20,21,22,23,24,30,31,32]);s.sort_unstable()}return Ok((sha,s))}
    Ok(match p{
        water_policy649::PROFILE=>(WATER,vec![33,39,40,44,45,46,47,48]),
        "original-water0-v1"|"original-water0-source45-grid26-mixed160-v1"=>(WATER,vec![33,39,40,44,45,46,47,48]),
        "original-water0-waves26-isolated-v1"=>(WATER,vec![33,39,40,43,44,45,46,47,48]),
        "standard-rock0-source-profile-v1"=>(ROCK,vec![39]),
        foliage_policy883::PROFILE=>(foliage_policy883::PS,foliage_policy883::NULLS.to_vec()),
        UNIT=>(ROCK,vec![30,31,32,33,39,40,41,44,45,46,47,48]),
        _=>return Err("unknown original caller profile".into()),
    })
}
fn validate_fixture(p:&str,sha:&str,f:&NullImageFixture)->Result<(),String>{
    let s=f.binding.checked_sub(32).ok_or("NULL binding namespace")?;
    let (dimension,layers)=if p==foliage_policy883::PROFILE{foliage_policy883::host_null_descriptor(s)}else if p==UNIT&&matches!(s,30..=32|41|44){("Texture2D",0)}else{match s{33=>("TextureCube",0),30..=32|39|40=>("Texture2DArray",1),45..=48=>("Texture3D",0),_=>("Texture2D",0)}};
    let (provenance,revision)=match p {
        water_policy649::PROFILE=>(water_policy649::CONTRACT,water_policy649::RULE),
        "original-water0-v1"=>("b3838a1c7e6e2e87af56c7b0fc27feba7c058df29b2961903e29afdeed7d6f55","source-water45-explicit-null-lightviews-v1"),
        "original-water0-waves26-isolated-v1"=>("b58166b9751741745c12695d8f5b4e32099239799d63532e406b52ccf4efac1a","waves26-strict-null-images-and-buffers-v1"),
        "original-water0-source45-grid26-mixed160-v1"=>("ee52435be416bc59f13402d3f2da3d203f84756eb36a5330e3240c8e645c4795","source-water45-controlled-waypoint-unit-mixed160-null-view-v1"),
        "standard-rock0-source-profile-v1"=>("21007827dddb55b6876967ba355139956ce450442caafe9032451e0f3a0b789f","standard-rock-sourcebank45-explicit-null39-unit-view-isnan-v2"),
        UNIT=>("21007827dddb55b6876967ba355139956ce450442caafe9032451e0f3a0b789f","sourcebank45-owned-unit-null-unobserved-canonical-wgsl-v2"),
        foliage_policy883::PROFILE=>(foliage_policy883::HOST,foliage_policy883::RULE),
        _ if s==39=>("bccd73046620c254dbda9b66c2782cf33d57302216e2f67e0115c7bd9cd85321","accepted177-explicit-null-t39-sample-results-v1"),
        _=>("1303ee4f8c7e7d33028ac125edebaaed563ca99f7eb753e30b785a0c66eb003c","accepted177-explicit-null-material-image-results-usage-v3"),
    };
    if f.profile!=p||f.original_dxbc_sha256!=sha||f.group!=1||f.original_register!=format!("ps:t{s}")||f.api!="D3D12-descriptor-table-null-SRV"||!f.resource_is_null||f.dimension!=dimension||f.format!="R32G32B32A32_FLOAT"||f.array_layers!=layers||f.mip_levels!=1||f.first_array_slice!=0||f.most_detailed_mip!=0||f.shader_component_mapping!=5768||f.plane_slice!=0||f.resource_min_lod_clamp_bits!=0||f.accepted_host_provenance_sha256!=provenance||f.rule_revision!=revision{return Err(format!("original descriptor/provenance mismatch t{s}"))}Ok(())
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct ProjectedOperation {pub function:String,pub expression:usize,pub source_register:u32,pub source_kind:String,pub result_type:String,pub source_result_semantics:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct UnreadResource {pub resource_id:u32,pub register:u32,pub original_resource_sha256:String,pub reason:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct GuardEmitRemoval {pub function:String,pub removed_expression_handles:Vec<usize>,pub removed_query_handles:Vec<usize>,pub query_expressions_unchanged:bool}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CallerProjectionReport {pub revision:String,pub evidence:CallerEvidence,pub operations:Vec<ProjectedOperation>,pub unread_resources:Vec<UnreadResource>,pub dead_guard_emits:Vec<GuardEmitRemoval>,pub unchanged_control_flow:bool,pub unchanged_global_declarations:bool,pub complete_resource_projection:bool,pub grants_current_host_authority:bool}

fn type_handle(module:&ir::Module,inner:&TypeInner)->Result<Handle<ir::Type>,String>{module.types.iter().find_map(|(h,t)|(&t.inner==inner).then_some(h)).ok_or_else(||format!("existing same-type zero missing: {inner:?}"))}
/// ZeroValue is pre-emitted by Naga. Remove ONLY its handle from Emit scopes;
/// preserve every other expression handle, statement, condition, branch and span.
pub(crate) fn constant_scopes(block:&mut ir::Block,zeros:&BTreeSet<usize>){
    let old=std::mem::take(block);
    for (mut s,span) in old.span_into_iter(){
        if let ir::Statement::Emit(range)=s {
            let mut run:Option<(Handle<E>,Handle<E>)>=None;
            for h in range {if zeros.contains(&h.index()){if let Some((a,b))=run.take(){block.push(ir::Statement::Emit(ir::Range::new_from_bounds(a,b)),span)}}else{run=Some((run.map_or(h,|r|r.0),h));}}
            if let Some((a,b))=run{block.push(ir::Statement::Emit(ir::Range::new_from_bounds(a,b)),span)}
            continue
        }
        match &mut s {ir::Statement::Block(b)=>constant_scopes(b,zeros),ir::Statement::If{accept,reject,..}=>{constant_scopes(accept,zeros);constant_scopes(reject,zeros)},ir::Statement::Switch{cases,..}=>{for c in cases{constant_scopes(&mut c.body,zeros)}},ir::Statement::Loop{body,continuing,..}=>{constant_scopes(body,zeros);constant_scopes(continuing,zeros)},_=>{}}
        block.push(s,span);
    }
}
#[derive(Clone,Copy)]enum FnKey{Ordinary(Handle<Function>),Entry(usize)}
fn literal_zero(f:&Function,h:Handle<E>)->bool{match f.expressions[h]{E::Literal(ir::Literal::U32(0)|ir::Literal::I32(0))=>true,E::Splat{value,..}=>literal_zero(f,value),_=>false}}
/// Exact public Iridia guarded_load shape: replace its final value, not the
/// inserted ArrayLength query or an arbitrary source query. Unrelated queries
/// remain live and make the later unread proof fail closed.
fn null_guarded_load(f:&Function,e:&E,globals:&BTreeMap<Handle<ir::GlobalVariable>,u32>)->Option<u32>{
    let E::Select{condition,accept,reject}=*e else{return None};
    if !literal_zero(f,reject){return None}
    let E::Load{pointer}=f.expressions[accept] else{return None};
    let E::Access{base,index:safe}=f.expressions[pointer] else{return None};
    let E::GlobalVariable(g)=f.expressions[base]else{return None};let slot=*globals.get(&g)?;
    if !matches!(slot,4|5|8){return None}
    // Builder::select splats the scalar bounds predicate for a vec4 typed load.
    let raw_condition=if let E::Splat{value,..}=f.expressions[condition]{value}else{condition};
    let E::Binary{op:ir::BinaryOperator::Less,left:index,right:len}=f.expressions[raw_condition]else{return None};
    if !matches!(f.expressions[len],E::ArrayLength(p) if p==base){return None}
    let E::Select{condition:c,accept:i,reject:z}=f.expressions[safe]else{return None};
    (c==raw_condition&&i==index&&literal_zero(f,z)).then_some(slot)
}
/// Called only after the complete guarded-load shape was matched above. This
/// contains only lifter-inserted bounds/load operations, never source address math.
fn guard_dependencies(f:&Function,e:&E)->Vec<Handle<E>>{
    let E::Select{condition,accept,..}=*e else{unreachable!()};
    let E::Load{pointer}=f.expressions[accept]else{unreachable!()};
    let E::Access{index:safe,..}=f.expressions[pointer]else{unreachable!()};
    let raw=if let E::Splat{value,..}=f.expressions[condition]{value}else{condition};
    let E::Binary{right:len,..}=f.expressions[raw]else{unreachable!()};
    let mut out=vec![len,raw,safe,pointer,accept];if condition!=raw{out.push(condition)}out
}
fn validate_image_declaration(p:&Program,g:Handle<ir::GlobalVariable>,slot:u32,profile:&str)->Result<(),String>{
    let v=&p.module.global_variables[g];
    if v.binding!=Some(ir::ResourceBinding{group:1,binding:slot}){return Err("caller projection must precede resource remapping".into())}
    let TypeInner::Image{dim,arrayed,class}=p.module.types[v.ty].inner else{return Err("NULL image source declaration is not image".into())};
    let expected=match slot{33=>(ir::ImageDimension::Cube,false),30..=32|39|40=>(ir::ImageDimension::D2,true),45..=48=>(ir::ImageDimension::D3,false),_=>(ir::ImageDimension::D2,false)};
    // UNIT 30..32 intentionally keeps source arrays, while the expected bound
    // descriptor is 2D. Only its separate consumed-byte gate permits it.
    if (dim,arrayed)!=expected{return Err(format!("original NULL image dimension changed t{slot}"))}
    let valid_class=if matches!(slot,33|39|40){matches!(class,ir::ImageClass::Depth{multi:false})}else{matches!(class,ir::ImageClass::Sampled{kind:ir::ScalarKind::Float,multi:false})};
    if !valid_class{return Err(format!("original NULL image sampling class changed t{slot} {profile}"))}Ok(())
}
pub(crate) fn apply_caller_projection(original:&mut Program,cap:&CallerCapability)->Result<CallerProjectionReport,String>{
    if cap.evidence.profile==background_policy677::PROFILE{return background_policy677::apply(original,cap).map_err(|e|e.to_string());}
    if cap.evidence.profile==standard_policy668::PROFILE{return standard_policy668::apply(original,cap).map_err(|e|e.to_string());}
    if cap.evidence.profile==standard_map1129::PROFILE{return standard_map1129::apply(original,cap).map_err(|e|e.to_string());}
    if cap.evidence.profile==water_policy649::PROFILE{water_policy649::check_cap(cap).map_err(|e|e.to_string())?;}
    let foliage=cap.evidence.profile==foliage_policy883::PROFILE;
    if foliage{foliage_policy883::evidence(&cap.evidence).map_err(|e|e.to_string())?;}
    let map_alpha=cap.evidence.profile==map_alpha_threshold1129::PROFILE;
    if map_alpha{map_alpha_threshold1129::evidence(&cap.evidence).map_err(|e|e.to_string())?;}
    // The VariantMaterial body authority (same structural typed-NULL rule).
    let map_variant=cap.evidence.profile==map_variant_body1153::PROFILE;
    if map_variant{map_variant_body1153::evidence(&cap.evidence).map_err(|e|e.to_string())?;}
    original.precision.validate(&original.module,original.provenance.source_hash.as_deref())?;
    let mut candidate=original.clone();let program=&mut candidate;
    if program.stage!=iridia::Stage::Pixel||program.provenance.source_hash.as_deref()!=Some(&format!("sha256:{}",cap.evidence.original_dxbc_sha256)){return Err("caller source/stage differs".into())}
    let image_wanted:BTreeSet<_>=cap.evidence.fixtures.iter().map(|f|f.binding-32).collect();
    let mut wanted=image_wanted.clone();for b in &cap.evidence.null_buffers{wanted.insert(b.validate()?);}
    let mut globals=BTreeMap::new();let mut resource_meta=BTreeMap::new();
    for r in &program.interface.resources {if let Some(d)=r.provenance.d3d {if d.class==D3dClass::ShaderResource&&wanted.contains(&d.register){if d.space!=0||r.count!=1{return Err("resource array/space".into())}let g=program.global(r.id).ok_or("missing source global")?;if image_wanted.contains(&d.register){validate_image_declaration(program,g,d.register,&cap.evidence.profile)?;}globals.insert(g,d.register);resource_meta.insert(d.register,(r.id.0,format!("{:x}",Sha256::digest(serde_json::to_vec(r).map_err(|e|e.to_string())?))));}}}
    if resource_meta.len()!=wanted.len(){return Err("caller declaration set incomplete".into())}
    for kind in [ir::ScalarKind::Float,ir::ScalarKind::Uint]{for inner in [TypeInner::Scalar(ir::Scalar{kind,width:4}),TypeInner::Vector{size:ir::VectorSize::Quad,scalar:ir::Scalar{kind,width:4}}]{program.module.types.insert(ir::Type{name:None,inner},ir::Span::UNDEFINED);}}
    let mut edits=vec![];let mut operations=vec![];let mut counts:BTreeMap<u32,(usize,usize,usize,usize)>=BTreeMap::new();let mut buffer_counts=BTreeMap::<u32,usize>::new();let mut guard_sets:Vec<(FnKey,String,BTreeSet<usize>)>=vec![];
    for (key,name,f) in program.module.functions.iter().map(|(h,f)|(FnKey::Ordinary(h),f.name.clone().unwrap_or_default(),f)).chain(program.module.entry_points.iter().enumerate().map(|(i,e)|(FnKey::Entry(i),e.name.clone(),&e.function))){
        let mut types=ir::front::Typifier::new();let ctx=ir::proc::ResolveContext::with_locals(&program.module,&f.local_variables,&f.arguments);
        let mut guards=BTreeSet::new();
        for (h,e) in f.expressions.iter(){
            if let Some(slot)=null_guarded_load(f,e,&globals){if cap.evidence.null_buffers.iter().any(|b|b.binding==32+slot){
                guards.extend(guard_dependencies(f,e).iter().map(|h|h.index()));
                types.grow(h,&f.expressions,&ctx).map_err(|e|e.to_string())?;let inner=types.get(h,&program.module.types);let zero=type_handle(&program.module,inner)?;edits.push((key,h,zero));*buffer_counts.entry(slot).or_default()+=1;operations.push(ProjectedOperation{function:name.clone(),expression:h.index(),source_register:slot,source_kind:"guarded-buffer-load".into(),result_type:format!("{inner:?}"),source_result_semantics:"DefinedMatchedNullZero".into()});continue
            }}
            let E::ImageSample{image,level,depth_ref,gather,..}=e else{continue};let E::GlobalVariable(g)=f.expressions[*image] else{continue};let Some(&slot)=globals.get(&g)else{continue};
            if gather.is_some(){return Err("unreviewed gather on NULL resource".into())}
            types.grow(h,&f.expressions,&ctx).map_err(|e|e.to_string())?;let inner=types.get(h,&program.module.types);let zero=type_handle(&program.module,inner)?;
            let c=counts.entry(slot).or_default();c.0+=1;let kind=match level {SampleLevel::Exact(_)=>{c.1+=1;"exact"},SampleLevel::Zero if depth_ref.is_some()=>{c.2+=1;"comparison-zero"},SampleLevel::Gradient{..}=>{c.3+=1;"gradient"},SampleLevel::Bias(_)=>"bias",SampleLevel::Auto=>"auto",_=>return Err("unreviewed sample level".into())};
            operations.push(ProjectedOperation{function:name.clone(),expression:h.index(),source_register:slot,source_kind:kind.into(),result_type:format!("{inner:?}"),source_result_semantics:if foliage{foliage_policy883::operation(slot,kind)?}else if cap.evidence.profile==UNIT&&matches!(slot,30..=32){"UnobservedUnderConsumedCount"}else{"DefinedMatchedNullZero"}.into()});edits.push((key,h,zero));
        }
        if !guards.is_empty(){guard_sets.push((key,name.clone(),guards));}
    }
    for &slot in &image_wanted {let c=counts.get(&slot).copied().unwrap_or_default();let expected=match slot{39=>(50,10,40,0),33|40=>(4,0,4,0),30|31=>(1,0,0,1),32=>(6,0,0,6),20..=24=>(1,0,0,0),43=>(1,0,0,0),41|44..=48=>(1,1,0,0),_=>return Err("unknown source operation rule".into())};if c!=expected{return Err(format!("exact reviewed operation set t{slot}: {c:?} != {expected:?}"))}}
    if !cap.evidence.null_buffers.is_empty()&&buffer_counts!=[(4,1),(5,3),(8,11)].into(){return Err(format!("exact original guarded-buffer fetch counts required: {buffer_counts:?}"))}
    // Work on a clone and commit only after all result forms and unread checks pass.
    let mut next=program.clone();for (key,h,zero)in edits{let f=match key{FnKey::Ordinary(h)=>next.module.functions.get_mut(h),FnKey::Entry(i)=>&mut next.module.entry_points[i].function};f.expressions[h]=E::ZeroValue(zero);}
    for (_,f) in next.module.functions.iter_mut(){let zeros=f.expressions.iter().filter_map(|(h,e)|matches!(e,E::ZeroValue(_)).then_some(h.index())).collect();constant_scopes(&mut f.body,&zeros)}
    for ep in &mut next.module.entry_points {let zeros=ep.function.expressions.iter().filter_map(|(h,e)|matches!(e,E::ZeroValue(_)).then_some(h.index())).collect();constant_scopes(&mut ep.function.body,&zeros)}
    let mut dead_guard_emits=vec![];
    for (key,name,guards)in guard_sets{
        let f=match key{FnKey::Ordinary(h)=>next.module.functions.get_mut(h),FnKey::Entry(i)=>&mut next.module.entry_points[i].function};
        let before=format!("{:?}",f.expressions);
        let removed=caller_guard_liveness::remove_dead_guard_emits(f,&guards)?;
        let queries=f.expressions.iter().filter_map(|(h,e)|(guards.contains(&h.index())&&matches!(e,E::ArrayLength(_))).then_some(h.index())).collect();
        if before!=format!("{:?}",f.expressions){return Err("guard cleanup changed an expression/query value".into())}
        dead_guard_emits.push(GuardEmitRemoval{function:name,removed_expression_handles:removed,removed_query_handles:queries,query_expressions_unchanged:true});
    }
    next.precision = original.precision.unchanged_arithmetic(&original.module,&next.module,iridia::precision::TransformKind::CallerNull)?;
    next.validate().map_err(|e|format!("projected module validation: {e:?}"))?;
    let unread=next.unread_resources().map_err(|e|e.to_string())?;let mut resources=vec![];
    for (&slot,(id,sha)) in &resource_meta {if !unread.contains(&iridia::ResourceId(*id)){return Err(format!("projected t{slot} remains entrypoint-read"))}resources.push(UnreadResource{resource_id:*id,register:slot,original_resource_sha256:sha.clone(),reason:if foliage{foliage_policy883::unread_reason(slot)}else if cap.evidence.profile==UNIT&&matches!(slot,30..=32){"UnobservedSourceBranch"}else{"MatchedNull"}.into()});}
    if map_alpha{map_alpha_threshold1129::coverage(&next,&cap.evidence).map_err(|e|e.to_string())?;}
    if map_variant{map_variant_body1153::coverage(&next,&cap.evidence).map_err(|e|e.to_string())?;}
    *original=next;Ok(CallerProjectionReport{revision:CALLER_REVISION.into(),evidence:cap.evidence.clone(),operations,unread_resources:resources,dead_guard_emits,unchanged_control_flow:true,unchanged_global_declarations:true,complete_resource_projection:cap.evidence.profile!="original-water0-waves26-isolated-v1"||cap.evidence.null_buffers.len()==3,grants_current_host_authority:false})
}

#[path="water_policy649.rs"] pub mod water_policy649;

#[path="standard_policy668.rs"] pub mod standard_policy668;
#[path="standard_map1129.rs"] pub mod standard_map1129;
#[path="map_alpha_threshold1129.rs"] pub mod map_alpha_threshold1129;
#[path="map_variant_body1153.rs"] pub mod map_variant_body1153;

/// The three-background capability types have no values in this program.
pub mod background_policy677 {
    use super::{CallerCapability, CallerProjectionReport};
    use crate::resource_contract::CallerContract;
    use crate::{Error, PreparedShader, Result, SourceShader, Stage};
    use serde_json::Value;
    pub const PROFILE: &str = "original-background630-active19-three677-v1";
    pub const RULE: &str = "background630-three-active19-guarded-null-v1";
    fn absent() -> Error {
        Error::Unsupported("Background677: not part of this build".into())
    }
    pub enum BackgroundDraw677 {}
    impl BackgroundDraw677 {
        pub fn physical_callers(&self) -> Result<[CallerContract; 2]> { match *self {} }
    }
    pub enum BackgroundCapability677 {}
    impl BackgroundCapability677 {
        pub(crate) fn seal(draw: &BackgroundDraw677, _: &SourceShader, _: &SourceShader) -> Result<Self> { match *draw {} }
        pub(crate) fn projection(&self) -> &CallerCapability { match *self {} }
        pub(crate) fn recheck(&self, _: &BackgroundDraw677, _: &SourceShader, _: &SourceShader) -> Result<()> { match *self {} }
        pub(crate) fn receipt(&self) -> Result<Value> { match *self {} }
    }
    pub(crate) fn contract(_: Stage, _: &CallerContract, _: bool) -> Result<()> {
        Err(absent())
    }
    pub(crate) fn apply(_: &mut iridia::Program, _: &CallerCapability) -> Result<CallerProjectionReport> {
        Err(absent())
    }
    pub(crate) fn cached(p: &PreparedShader) -> Result<()> {
        if p.caller_contract()["profile"] != PROFILE {
            return if p.caller_contract()["background677"].is_null() {
                Ok(())
            } else {
                Err(Error::Unsupported("Background677: Standard receipt on another profile".into()))
            };
        }
        Err(absent())
    }
}

#[path="foliage_policy883.rs"] pub mod foliage_policy883;
