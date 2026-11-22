//! Independent stock-Naga reflection of serialized prepared artifacts.
//! This checks internal consistency, not original semantic equivalence or GPU
//! resource/current-frame authority.
use crate::{Error, PreparedPair, PreparedShader, Result, Stage, TargetContract};
use crate::resource_contract::{BindingManifest, CallerContract, ResourceContractKind as K, RegisterClass, UnreadReason, RESOURCE_REVISION};
use crate::stage_link::{DrawContract,DrawManifest,LinkManifest,DRAW_REVISION,LINK_REVISION};
use naga::{AddressSpace,Binding,ImageClass,ImageDimension,ScalarKind,TypeInner};
use std::collections::{BTreeMap,BTreeSet};

fn bad(s:impl Into<String>)->Error {Error::Identity(format!("prepared reflection: {}",s.into()))}
fn module(p:&PreparedShader)->Result<naga::Module> {
    p.payload().module()
}
fn expected_stage(s:Stage)->Result<naga::ShaderStage> {match s {
    Stage::Vertex=>Ok(naga::ShaderStage::Vertex),Stage::Fragment=>Ok(naga::ShaderStage::Fragment),Stage::Compute=>Ok(naga::ShaderStage::Compute),
    _=>Err(bad("hull/domain cannot be WGSL entry stages")),
}}
fn stage_name(s:Stage)->&'static str {match s {Stage::Vertex=>"vertex",Stage::Fragment=>"fragment",Stage::Compute=>"compute",Stage::Hull=>"hull",Stage::Domain=>"domain"}}
// A SPIR-V buffer block may wrap exactly one byte-identical array. This
// does not establish buffer residency, source views, access, or host authority.
fn buffer_array(m:&naga::Module,ty:naga::Handle<naga::Type>)->Option<naga::Handle<naga::Type>> {
    match &m.types[ty].inner {
        TypeInner::Array{..}=>Some(ty),
        TypeInner::Struct{members,span} if members.len()==1 && members[0].offset==0 && members[0].binding.is_none()=>{
            let inner=members[0].ty;
            let TypeInner::Array{size,stride,..}=m.types[inner].inner else{return None};
            let count=match size {naga::ArraySize::Constant(n)=>n.get(),naga::ArraySize::Dynamic=>1,_=>return None};
            (stride.checked_mul(count)==Some(*span)).then_some(inner)
        },
        _=>None,
    }
}
fn words(m:&naga::Module,ty:naga::Handle<naga::Type>,stride:u32,vector:bool)->bool {
    let Some(ty)=buffer_array(m,ty) else{return false};
    let TypeInner::Array{base,stride:s,..}=m.types[ty].inner else{return false};
    s==stride && if vector {matches!(m.types[base].inner,TypeInner::Vector{size:naga::VectorSize::Quad,scalar:naga::Scalar::U32})}else{matches!(m.types[base].inner,TypeInner::Scalar(naga::Scalar::U32))}
}
// Source read-only buffer preparation requires runtime arrays, including after
// SPIR-V block wrapping. A fixed array can change bounds and length semantics
// even when its byte size happens to equal the declared source view.
fn runtime_words(m:&naga::Module,ty:naga::Handle<naga::Type>,stride:u32,vector:bool)->bool {
    let Some(ty)=buffer_array(m,ty) else{return false};
    matches!(m.types[ty].inner,TypeInner::Array{size:naga::ArraySize::Dynamic,..})
        && words(m,ty,stride,vector)
}
fn uniform(m:&naga::Module,g:&naga::GlobalVariable,bytes:u32)->bool {
    if g.space!=AddressSpace::Uniform{return false}
    // SPIR-V block transport wraps the same byte-exact array. Admit only one
    // undecorated member at offset zero with no extra or trailing bytes.
    let ty=match &m.types[g.ty].inner {
        TypeInner::Struct{members,span} if members.len()==1 && *span==bytes
            && members[0].offset==0 && members[0].binding.is_none()=>members[0].ty,
        TypeInner::Struct{..}=>return false,
        _=>g.ty,
    };
    if !matches!(m.types[ty].inner,TypeInner::Array{..}) || !words(m,ty,16,true){return false}
    matches!(m.types[ty].inner,TypeInner::Array{size:naga::ArraySize::Constant(n),..} if n.get().checked_mul(16)==Some(bytes))
}
fn dim(d:ImageDimension)->&'static str {match d {ImageDimension::D1=>"d1",ImageDimension::D2=>"d2",ImageDimension::D3=>"d3",ImageDimension::Cube=>"cube"}}

pub(crate) fn storage_buffer_transport(wgsl:&str,caller:&CallerContract)->Result<serde_json::Value>{
    let m=naga::front::wgsl::parse_str(wgsl).map_err(|e|bad(e.emit_to_string(wgsl)))?;
    storage_buffer_transport_module(&m,caller)
}
pub(crate) fn storage_buffer_transport_module(m:&naga::Module,caller:&CallerContract)->Result<serde_json::Value>{
    let info=naga::valid::Validator::new(naga::valid::ValidationFlags::all(),naga::valid::Capabilities::all()).validate(m).map_err(|e|bad(format!("storage transport module: {e:?}")))?;
    let mut out=vec![];
    for r in &caller.resources {if let K::StorageBuffer{access,..}=&r.resource {
        let (h,g)=m.global_variables.iter().find(|(_,g)|g.binding==Some(naga::ResourceBinding{group:r.group,binding:r.binding})).ok_or_else(||bad("storage transport binding absent"))?;
        let AddressSpace::Storage{access:actual}=g.space else{return Err(bad("storage transport address space"))};
        let actual_name=if actual==naga::StorageAccess::LOAD{"read"}else if actual==naga::StorageAccess::LOAD|naga::StorageAccess::STORE{"read_write"}else if actual==naga::StorageAccess::STORE{"write"}else{return Err(bad("unsupported final storage buffer access"))};
        let read=info.get_entry_point(0)[h].contains(naga::valid::GlobalUse::READ);
        let rule=match access.as_str(){"write" if actual_name=="write"&&!read=>"same-write-access","write" if actual_name=="read_write"&&!read=>"wgsl-write-buffer-binding-without-loads", "read" if actual_name=="read"=>"same-read-access", "read_write" if actual_name=="read_write"=>"same-read-write-access",_=>return Err(bad("source/WGSL storage access or unexpected load differs"))};
        out.push(serde_json::json!({"group":r.group,"binding":r.binding,"source_access":access,"wgsl_access":actual_name,"entrypoint_reads":read,"rule":rule}));
    }}
    Ok(serde_json::json!(out))
}

pub(crate) fn validate_prepared(p:&PreparedShader)->Result<()> {
    let authored830=crate::authored_light830::validate_stage(p)?;
    if authored830.is_none(){
    crate::caller_projection::background_policy677::cached(p)?;
    crate::caller_projection::standard_policy668::cached(p)?;
    crate::caller_projection::standard_map1129::cached(p)?;
    crate::caller_projection::water_policy649::check_cached_stage(p)?;
    crate::caller_projection::foliage_policy883::cached(p)?;
    }
    let m=module(p)?;
    let info=naga::valid::Validator::new(naga::valid::ValidationFlags::all(),naga::valid::Capabilities::all()).validate(&m).map_err(|e|bad(format!("stock module: {e:?}")))?;
    let stage=p.identity().source.stage;
    let manifest=p.manifest();
    if manifest["gpu_verified"]!=false||manifest["stock_naga_validated"]!=true||manifest["outstanding_notes"].as_array().is_none_or(|a|!a.is_empty()){return Err(bad("CPU preparation qualification/unknown notes"))}
    if m.entry_points.len()!=1 || m.entry_points[0].stage!=expected_stage(stage)?
        || manifest["stage"].as_str()!=Some(stage_name(stage))
        || manifest["entry_point"].as_str()!=Some(m.entry_points[0].name.as_str()) {return Err(bad("entry stage/name differs from source/manifest"))}
    let target:TargetContract=serde_json::from_value(p.target_capabilities().clone())?;
    target.validate()?;
    if stage==Stage::Compute {
        let limits=target.compute.as_ref().ok_or_else(||bad("compute target limits missing"))?;
        let size=m.entry_points[0].workgroup_size;
        let invocations=size.iter().try_fold(1u64,|n,v|n.checked_mul(u64::from(*v))).ok_or_else(||bad("compute workgroup overflow"))?;
        let memory=m.global_variables.iter().filter(|(h,g)|g.space==AddressSpace::WorkGroup&&!info.get_entry_point(0)[*h].is_empty()).try_fold(0u64,|n,(_,g)|n.checked_add(u64::from(m.types[g.ty].inner.size(m.to_ctx())).div_ceil(16)*16)).ok_or_else(||bad("compute memory overflow"))?;
        if invocations>u64::from(limits.max_invocations_per_workgroup)||size.iter().zip(limits.max_workgroup_size).any(|(a,b)|*a>b)||memory>limits.max_workgroup_storage_size{return Err(bad("actual WGSL exceeds compute target limits"))}
        if p.manifest()["compute_contract"]!=serde_json::json!({"workgroup_size":size,"invocations":invocations,"workgroup_memory_bytes":memory,"required_limits_checked":true,"actual_device_attested":false}){return Err(bad("compute module/manifest report differs"))}
    }
    let caller:CallerContract=serde_json::from_value(p.caller_contract()["resources"].clone())?;
    // Compute stages always report their storage transport; a fragment stage reports it exactly
    // when its caller binds a (source read-only, checked below) UAV buffer, and never otherwise.
    let fragment_storage=stage==Stage::Fragment&&caller.resources.iter().any(|r|matches!(r.resource,K::StorageBuffer{..}));
    if stage==Stage::Compute||fragment_storage {if manifest["storage_buffer_transport"]!=storage_buffer_transport_module(&m,&caller)? {return Err(bad("storage buffer transport report differs"))}}
    else if manifest.get("storage_buffer_transport").is_some(){return Err(bad("storage buffer transport report without a storage binding"))}
    target.resources(&caller)?;
    let bindings:BindingManifest=serde_json::from_value(manifest["bindings"].clone())?;
    let projection:Option<crate::caller_projection::CallerProjectionReport>=if manifest["caller_projection"].is_null(){None}else{Some(serde_json::from_value(manifest["caller_projection"].clone())?)};
    if let Some(r)=&projection {
        if stage!=Stage::Fragment||r.revision!=crate::caller_projection::CALLER_REVISION||!r.complete_resource_projection||!r.unchanged_control_flow||!r.unchanged_global_declarations||r.grants_current_host_authority||r.evidence.grants_current_host_authority
            ||r.evidence.original_dxbc_sha256!=p.identity().source.dxbc_sha256||authored830.as_deref().unwrap_or(p.caller_contract()["profile"].as_str().unwrap_or(""))!=r.evidence.profile||p.caller_contract()["projection"]!=serde_json::to_value(&r.evidence)? {return Err(bad("source caller projection report/qualification differs"))}
        let unique=r.unread_resources.iter().map(|r|r.resource_id).collect::<BTreeSet<_>>();if unique.len()!=r.unread_resources.len(){return Err(bad("duplicate projection unread resource"))}
        if r.evidence.null_buffers.is_empty(){if !r.dead_guard_emits.is_empty(){return Err(bad("dead guard cleanup outside original NULL buffer family"))}}
        else{
            if r.evidence.profile!="original-water0-waves26-isolated-v1"||r.evidence.null_buffers.len()!=3{return Err(bad("dead guard cleanup source family"))}
            let mut handles=BTreeSet::new();let mut queries=BTreeSet::new();
            for e in &r.dead_guard_emits {if !e.query_expressions_unchanged{return Err(bad("source guard query expressions changed"))}for h in &e.removed_expression_handles{if !handles.insert((e.function.clone(),*h)){return Err(bad("duplicate removed source guard handle"))}}for h in &e.removed_query_handles{if !e.removed_expression_handles.contains(h)||!queries.insert((e.function.clone(),*h)){return Err(bad("source guard query coverage differs"))}}}
            if handles.len()!=86||queries.len()!=15{return Err(bad("exact original NULL guard Emit/query counts differ"))}
        }
    }
    if caller.revision!=RESOURCE_REVISION || bindings.revision!=RESOURCE_REVISION
        || bindings.host_resource_presence_proved || !bindings.standalone_exact_view_rebase_required
        || caller.resources.len()!=bindings.resources.len() {return Err(bad("resource revision/count/authority qualification"))}
    let mut actual=BTreeMap::new();
    for (h,g) in m.global_variables.iter() {
        if let Some(b)=g.binding.as_ref() {
            if actual.insert((b.group,b.binding),(h,g)).is_some(){return Err(bad("duplicate WGSL resource binding"))}
        }
    }
    let mut expected=BTreeSet::new();let mut registers=BTreeSet::new();
    let declarations=manifest["source_declarations"].as_array().ok_or_else(||bad("original declaration records"))?;
    if declarations.len()!=caller.resources.len(){return Err(bad("original declaration count differs"))}
    for c in &caller.resources {
        if c.count!=1 || c.space!=0 || !registers.insert((c.class,c.space,c.register)) || !expected.insert((c.group,c.binding)){return Err(bad("duplicate/unadmitted caller source or target"))}
        let rows=bindings.resources.iter().filter(|b|b.class==c.class&&b.space==c.space&&b.register==c.register).collect::<Vec<_>>();
        if rows.len()!=1 || rows[0].group!=c.group || rows[0].binding!=c.binding || rows[0].resource!=c.resource {return Err(bad("manifest and caller bindings differ"))}
        let (global,g)=*actual.get(&(c.group,c.binding)).ok_or_else(||bad(format!("caller binding missing from final module: stage {stage:?}, {}/{}, contract {:?}; present {:?}",c.group,c.binding,c.resource,actual.keys().collect::<Vec<_>>())))?;
        let row=rows[0];
        let decl=declarations.iter().filter(|d|d["resource_id"].as_u64()==Some(u64::from(row.source_resource_id))).collect::<Vec<_>>();
        if decl.len()!=1{return Err(bad("original declaration id is absent/duplicated"))}
        let text=decl[0]["original_serialized_declaration"].as_str().ok_or_else(||bad("declaration bytes"))?;
        if decl[0]["sha256"].as_str()!=Some(crate::sha256(text.as_bytes()).as_str()){return Err(bad("declaration serialization checksum"))}
        let source_decl:serde_json::Value=serde_json::from_str(text)?;
        if source_decl["id"].as_u64()!=Some(u64::from(row.source_resource_id)) || source_decl["provenance"]["d3d"]["register"].as_u64()!=Some(u64::from(c.register))
            || source_decl["provenance"]["d3d"]["space"].as_u64()!=Some(u64::from(c.space)) || source_decl["provenance"]["d3d"]["class"]!=serde_json::to_value(c.class)? {return Err(bad("original declaration register metadata differs"))}
        if let Some(resources)=manifest["prepared_interface"]["resources"].as_array() {
            let matching=resources.iter().filter(|r|r["id"].as_u64()==Some(u64::from(row.source_resource_id))).collect::<Vec<_>>();
            if matching.len()!=1 || matching[0]!=&source_decl{return Err(bad("prepared/source declaration metadata differs"))}
        }else{return Err(bad("prepared interface resources absent"))}
        let ty=&m.types[g.ty].inner;
        let valid=match &c.resource {
            K::ConstantBuffer{bound_bytes}=>uniform(&m,g,*bound_bytes),
            K::RawBuffer{view}=>{
                view.range().map_err(bad)?;
                matches!(g.space,AddressSpace::Storage{access} if access==naga::StorageAccess::LOAD)
                    &&runtime_words(&m,g.ty,4,false)&&view.source_stride_bytes==4
            },
            K::StructuredBuffer{stride_bytes,view}=>{
                view.range().map_err(bad)?;
                matches!(g.space,AddressSpace::Storage{access} if access==naga::StorageAccess::LOAD)
                    &&runtime_words(&m,g.ty,4,false)&&u64::from(*stride_bytes)==view.source_stride_bytes
            },
            K::TypedBuffer{format,view}=>{
                view.range().map_err(bad)?;
                matches!(g.space,AddressSpace::Storage{access} if access==naga::StorageAccess::LOAD)
                    &&runtime_words(&m,g.ty,16,true)&&view.source_stride_bytes==16
                    &&matches!(format.as_str(),"R32G32B32A32_FLOAT"|"R32G32B32A32_UINT"|"R32G32B32A32_SINT")
            },
            K::Texture{dimension,arrayed,multisampled,sample}=>{
                g.space==AddressSpace::Handle && match ty {
                    TypeInner::Image{dim:d,arrayed:a,class} if dim(*d)==dimension && a==arrayed => match class {
                        ImageClass::Sampled{kind,multi}=>multi==multisampled && match sample.as_str(){"float"=>*kind==ScalarKind::Float,"uint"=>*kind==ScalarKind::Uint,"sint"=>*kind==ScalarKind::Sint,_=>false},
                        ImageClass::Depth{multi}=>multi==multisampled&&sample=="depth",
                        _=>false,
                    },
                    _=>false,
                }
            },
            K::Sampler{comparison}=>g.space==AddressSpace::Handle&&matches!(ty,TypeInner::Sampler{comparison:c} if c==comparison),
            K::StorageBuffer{access,stride_bytes,view}=>{
                // A pixel shader may read a UAV buffer it never writes (D3D `ld_structured`/`ld_raw`
                // from `u#`). Its WGSL form is a read-only storage word array, the transport the
                // admitted SRV buffers use. D3D returns 0 for an out-of-range element; the lift states
                // that guard explicitly (index < arrayLength, else 0) over the caller's exact view
                // range. Stores/atomics in graphics stages stay refused.
                if stage==Stage::Vertex{return Err(bad("vertex storage-buffer transport is not admitted"))}
                if stage==Stage::Fragment&&access!="read"{return Err(bad("fragment storage-buffer transport admits only source read-only access"))}
                view.range().map_err(bad)?;
                let a=match access.as_str(){"read"=>naga::StorageAccess::LOAD,"write"=>naga::StorageAccess::STORE,"read_write"=>naga::StorageAccess::LOAD|naga::StorageAccess::STORE,_=>return Err(bad("unknown storage access"))};
                let array=buffer_array(&m,g.ty).is_some_and(|t|matches!(m.types[t].inner,TypeInner::Array{base,size:naga::ArraySize::Dynamic,stride:4} if matches!(m.types[base].inner,TypeInner::Scalar(naga::Scalar::U32)|TypeInner::Atomic(naga::Scalar::U32))));
                let actual=if access=="write" && matches!(g.space,AddressSpace::Storage{access:x} if x==naga::StorageAccess::LOAD|naga::StorageAccess::STORE){naga::StorageAccess::LOAD|naga::StorageAccess::STORE}else{a};
                source_decl["kind"]["storage"]["access"]==*access&&matches!(g.space,AddressSpace::Storage{access:x} if x==actual) && array && u64::from(*stride_bytes)==view.source_stride_bytes
            },
            K::StorageImage{dimension,arrayed,access,format}=>{
                let a=match access.as_str(){"read"=>naga::StorageAccess::LOAD,"write"=>naga::StorageAccess::STORE,"read_write"=>naga::StorageAccess::LOAD|naga::StorageAccess::STORE,_=>return Err(bad("unknown storage image access"))};
                g.space==AddressSpace::Handle&&matches!(ty,TypeInner::Image{dim:d,arrayed:x,class:ImageClass::Storage{format:f,access:y}} if dim(*d)==dimension&&x==arrayed&&*y==a&&format!("{f:?}")==*format)
            },
            K::UnreadDeclaration{original_resource_sha256,reason}=>{
                let report=projection.as_ref().ok_or_else(||bad("unread binding lacks source caller projection"))?;
                let reason_matches=if c.class==RegisterClass::Sampler { *reason==UnreadReason::UnusedProjectionDependency }else{
                    report.unread_resources.iter().any(|r|r.resource_id==row.source_resource_id&&r.register==c.register&&r.original_resource_sha256==*original_resource_sha256&&match reason {UnreadReason::MatchedNull=>r.reason=="MatchedNull",UnreadReason::UnobservedSourceBranch=>r.reason=="UnobservedSourceBranch",UnreadReason::UnusedProjectionDependency=>false})
                };
                reason_matches&&!row.physical_binding_required&&info.get_entry_point(0)[global].is_empty()&&decl[0]["sha256"].as_str()==Some(original_resource_sha256.as_str())
            },
        };
        if row.physical_binding_required==matches!(c.resource,K::UnreadDeclaration{..}){return Err(bad("physical-binding qualification differs from resource contract"))}
        if !valid{return Err(bad(format!("actual WGSL type/layout disagrees at {}/{}: contract {:?}; actual {:?} {:?}; entry usage {:?}",c.group,c.binding,c.resource,g.space,ty,info.get_entry_point(0)[global])))}
    }
    if let Some(report)=&projection {
        let expected_ids=bindings.resources.iter().filter(|r|r.class!=RegisterClass::Sampler&&matches!(r.resource,K::UnreadDeclaration{..})).map(|r|r.source_resource_id).collect::<BTreeSet<_>>();
        if expected_ids!=report.unread_resources.iter().map(|r|r.resource_id).collect(){return Err(bad("projection report does not exactly cover unread source declarations"))}
    }
    let requires_typed_null=bindings.resources.iter().filter(|r|matches!(r.resource,K::UnreadDeclaration{reason:UnreadReason::MatchedNull,..})).any(|r|declarations.iter().filter(|d|d["resource_id"].as_u64()==Some(u64::from(r.source_resource_id))).any(|d|serde_json::from_str::<serde_json::Value>(d["original_serialized_declaration"].as_str().unwrap_or("")).is_ok_and(|v|v["kind"]["buffer"].get("typed").is_some())));
    if requires_typed_null&&manifest["typed_null_note"].is_null(){return Err(bad("original matched NULL typed declaration lacks its format-obligation resolution"))}
    if !manifest["typed_null_note"].is_null(){
        let typed:crate::typed_null_note::TypedNullNoteReport=serde_json::from_value(manifest["typed_null_note"].clone())?;
        let projection=projection.as_ref().ok_or_else(||bad("typed NULL note lacks source projection"))?;
        if stage!=Stage::Fragment||typed.revision!=crate::typed_null_note::TYPED_NULL_NOTE_REVISION||typed.shader_values_or_ir_changed||typed.grants_current_host_authority||typed.resolved_notes.is_empty()
            ||projection.evidence.profile!="original-water0-waves26-isolated-v1"||projection.evidence.original_dxbc_sha256!="c3ac426052a491b82f909e73f22ef04f327c7dec1c7230dd614eff2ff3246f27"{return Err(bad("typed NULL note scope/qualification"))}
        let mut physical=vec![];let mut matched_null=vec![];
        for decl in declarations {
            let source:serde_json::Value=serde_json::from_str(decl["original_serialized_declaration"].as_str().ok_or_else(||bad("typed declaration bytes"))?)?;
            if source["kind"]["buffer"].get("typed").is_none(){continue}
            let id=u32::try_from(source["id"].as_u64().ok_or_else(||bad("typed declaration id"))?).map_err(|_|bad("typed id range"))?;
            let row=bindings.resources.iter().find(|r|r.source_resource_id==id).ok_or_else(||bad("typed binding absent"))?;
            match &row.resource {K::TypedBuffer{..} if row.physical_binding_required=>physical.push(id),K::UnreadDeclaration{reason:UnreadReason::MatchedNull,..} if !row.physical_binding_required&&row.register==8&&row.group==1&&row.binding==40=>matched_null.push(id),_=>return Err(bad("typed note has neither physical view nor original matched NULL"))}
        }
        if physical!=typed.physical_typed_resource_ids||matched_null!=typed.matched_null_typed_resource_ids||matched_null.is_empty(){return Err(bad("typed note exact resource coverage differs"))}
        let ops=projection.operations.iter().filter(|o|o.source_register==8).collect::<Vec<_>>();let unique=ops.iter().map(|o|(&o.function,o.expression)).collect::<BTreeSet<_>>();
        if ops.len()!=11||unique.len()!=11||ops.iter().any(|o|o.source_kind!="guarded-buffer-load"||o.source_result_semantics!="DefinedMatchedNullZero"){return Err(bad("typed NULL source operation coverage differs"))}
        let mut all=physical;all.extend(matched_null);all.sort_unstable();
        for r in &typed.resolved_notes {if r.rule!=typed.revision||r.source_resource_ids!=all||r.original_note!="a typed buffer view is read as four 32-bit lanes per element: converting from the view's format is the binding's"||!manifest["original_notes"].as_array().is_some_and(|a|a.iter().any(|v|v==&r.original_note)){return Err(bad("typed note resolution/provenance differs"))}}
    }
    let draw:Option<DrawManifest>=serde_json::from_value(manifest["draw"].clone())?;
    let draw_contract:Option<DrawContract>=serde_json::from_value(p.caller_contract()["draw"].clone())?;
    if let Some(d)=draw {
        let c=draw_contract.ok_or_else(||bad("draw transport has no caller contract"))?;
        if stage!=Stage::Vertex || c.revision!=DRAW_REVISION || d.revision!=DRAW_REVISION || !crate::stage_link::supported_topology(&c.topology)
            || d.group!=c.base_group || d.binding!=c.base_binding || d.uniform_bytes!=16 || d.first_instance_offset!=0
            || !d.source_body_unchanged || d.normalization!="wrapping-u32(instance_index-first_instance)"
            || d.group>=target.max_bind_groups || d.binding>=target.max_bindings_per_bind_group || target.max_uniform_buffer_binding_size<16
            || !expected.insert((d.group,d.binding)) {return Err(bad("draw transport contract/qualification"))}
        let (_,g)=*actual.get(&(d.group,d.binding)).ok_or_else(||bad("draw uniform missing from WGSL"))?;
        if !uniform(&m,g,16){return Err(bad("draw uniform layout differs"))}
    } else if stage!=Stage::Vertex && draw_contract.is_some(){return Err(bad("nonvertex draw transport"))}
    if expected!=actual.keys().copied().collect(){return Err(bad("extra/missing WGSL declarations versus caller/draw"))}
    Ok(())
}

#[derive(Clone,Debug,PartialEq,Eq)]
struct Io {ty:String,interpolation:Option<naga::Interpolation>,sampling:Option<naga::Sampling>,name:String}
fn io_type(m:&naga::Module,h:naga::Handle<naga::Type>)->Result<String> {
    let (s,n)=match m.types[h].inner {TypeInner::Scalar(s)=>(s,1),TypeInner::Vector{size,scalar}=>(scalar,size as u8),_=>return Err(bad("unsupported entry varying type"))};
    Ok(format!("{:?}:{}:{n}",s.kind,s.width))
}
fn collect(m:&naga::Module,ty:naga::Handle<naga::Type>,binding:Option<&Binding>,name:&str,out:&mut BTreeMap<u32,Io>)->Result<()> {
    match binding {
        Some(Binding::Location{location,interpolation,sampling,..})=>{
            if out.insert(*location,Io{ty:io_type(m,ty)?,interpolation:*interpolation,sampling:*sampling,name:name.into()}).is_some(){return Err(bad("duplicate entry location"))}
        },
        Some(Binding::BuiltIn(_))=>{},
        None=>if let TypeInner::Struct{members,..}=&m.types[ty].inner {
            for v in members {collect(m,v.ty,v.binding.as_ref(),v.name.as_deref().unwrap_or(""),out)?}
        }else{return Err(bad("unbound entry varying"))},
    }
    Ok(())
}
pub(crate) fn validate_pair_linkage(p:&PreparedPair)->Result<()> {
    validate_graphics_linkage(&p.vertex,&p.fragment,&p.linkage)
}
pub(crate) fn validate_graphics_linkage(vertex:&PreparedShader,fragment:&PreparedShader,linkage:&LinkManifest)->Result<()> {
    // The vertex stage's draw contract and the linkage record one topology.
    let draw_topology=vertex.caller_contract()["draw"]["topology"].as_str();
    if linkage.revision!=LINK_REVISION || !crate::stage_link::supported_topology(&linkage.topology)
        || draw_topology.is_some_and(|t|t!=linkage.topology) || linkage.padding_allowed || !linkage.emitted_notes.is_empty(){return Err(bad("typed linkage revision/topology/notes"))}
    let rows=source_linkage(vertex,fragment,linkage)?;
    // The interface is checked on the payload the backend consumes: WGSL text, or SPIR-V words.
    match (vertex.payload().kind(),fragment.payload().kind()) {
        ("wgsl","wgsl")=>wgsl_linkage(vertex,fragment,&rows),
        (a,b) if a!="wgsl"&&b!="wgsl"=>spirv_linkage(vertex,fragment,&rows),
        _=>Err(bad("mixed WGSL/SPIR-V stage payloads")),
    }
}
type Rows<'a>=Vec<(&'a crate::stage_link::LinkedVarying,serde_json::Value)>;
/// Each linked semantic against both stages' prepared source signatures, independent of the
/// payload form. Returns the linkage rows with the consumer's source input record.
fn source_linkage<'a>(vertex:&PreparedShader,fragment:&PreparedShader,linkage:&'a LinkManifest)->Result<Rows<'a>> {
    let outputs=vertex.manifest()["prepared_interface"]["outputs"].as_array().ok_or_else(||bad("prepared vertex interface outputs"))?;
    let inputs=fragment.manifest()["prepared_interface"]["inputs"].as_array().ok_or_else(||bad("prepared fragment interface inputs"))?;
    let selected=|a:&Vec<serde_json::Value>,name:&str,index:u32|a.iter().filter(|v|v["system"].is_null()&&v["label"]["name"].as_str().is_some_and(|s|s.eq_ignore_ascii_case(name))&&v["label"]["index"].as_u64()==Some(u64::from(index))).cloned().collect::<Vec<_>>();
    let mut seen=BTreeSet::new();let mut rows=vec![];
    for row in &linkage.consumed {
        if !seen.insert((row.semantic.to_ascii_uppercase(),row.index)){return Err(bad("duplicate consumed semantic report"))}
        let a=selected(outputs,&row.semantic,row.index);let b=selected(inputs,&row.semantic,row.index);
        if a.len()!=1||b.len()!=1{return Err(bad("linked semantic absent/duplicate in prepared interfaces"))}
        let a=&a[0];let b=&b[0];
        let bits=|v:&serde_json::Value,k:&str|v["provenance"]["d3d"][k].as_u64().ok_or_else(||bad("source signature provenance"));
        if b["used"].as_bool()!=Some(true) || a["value"]!=b["value"] || b["value"]["width"].as_u64()!=Some(u64::from(row.lanes))
            || !b["value"]["scalar"].as_str().is_some_and(|s|s.eq_ignore_ascii_case(&row.scalar))
            || bits(a,"register")?!=u64::from(row.producer_register) || bits(b,"register")?!=u64::from(row.consumer_register)
            || (bits(b,"mask")?&bits(b,"read_write_mask")?)!=u64::from(row.required_mask)
            || (u64::from(row.required_mask)&!(bits(a,"mask")?&!bits(a,"read_write_mask")?))!=0 {return Err(bad("linked source signature report differs"))}
        rows.push((row,b.clone()));
    }
    let expected=inputs.iter().filter(|v|v["used"].as_bool()==Some(true)&&v["system"].is_null()).count();
    if expected!=seen.len(){return Err(bad("consumed source semantic missing from linkage report"))}
    Ok(rows)
}
/// The source interpolation of a consumed input, checked against the report's rendering of it.
fn source_interpolation(b:&serde_json::Value,row:&crate::stage_link::LinkedVarying)->Result<(naga::Interpolation,naga::Sampling)> {
    let (im,it)=match b["interp"]["mode"].as_str(){Some("perspective")=>(naga::Interpolation::Perspective,"Perspective"),Some("linear")=>(naga::Interpolation::Linear,"Linear"),Some("flat")=>(naga::Interpolation::Flat,"Flat"),_=>return Err(bad("missing source interpolation mode"))};
    let (sample,st)=match b["interp"]["sampling"].as_str(){Some("center")=>(naga::Sampling::Center,"Center"),Some("centroid")=>(naga::Sampling::Centroid,"Centroid"),Some("sample")=>(naga::Sampling::Sample,"Sample"),_=>return Err(bad("missing source interpolation sampling"))};
    if im==naga::Interpolation::Flat&&sample!=naga::Sampling::Center{return Err(bad("unimplemented flat sample qualifier"))}
    if row.interpolation!=format!("Interpolation {{ mode: {it}, sampling: {st} }}"){return Err(bad("source/report interpolation differs"))}
    Ok((im,sample))
}
fn wgsl_linkage(vertex:&PreparedShader,fragment:&PreparedShader,rows:&Rows<'_>)->Result<()> {
    let v=module(vertex)?;let f=module(fragment)?;
    let mut producer=BTreeMap::new();let mut consumer=BTreeMap::new();
    let r=v.entry_points[0].function.result.as_ref().ok_or_else(||bad("vertex result absent"))?;
    collect(&v,r.ty,r.binding.as_ref(),"",&mut producer)?;
    for a in &f.entry_points[0].function.arguments {collect(&f,a.ty,a.binding.as_ref(),a.name.as_deref().unwrap_or(""),&mut consumer)?}
    for (loc,c) in &consumer {
        let a=producer.get(loc).ok_or_else(||bad("fragment location has no vertex output"))?;
        if a.ty!=c.ty || a.interpolation!=c.interpolation || a.sampling!=c.sampling {return Err(bad("actual WGSL stage location/type/interpolation mismatch"))}
    }
    for (row,b) in rows {
        let name=format!("{}{}",row.semantic,row.index);
        let c=consumer.values().filter(|i|i.name.trim_end_matches('_').eq_ignore_ascii_case(&name)).collect::<Vec<_>>();
        if c.len()!=1{return Err(bad("semantic cannot be tied to actual WGSL input"))}
        let scalar=match row.scalar.to_ascii_lowercase().as_str(){"float"=>"Float","uint"=>"Uint","sint"=>"Sint",_=>return Err(bad("unsupported linkage scalar"))};
        if c[0].ty!=format!("{scalar}:4:{}",row.lanes){return Err(bad("actual WGSL semantic type differs from report"))}
        let (im,sample)=source_interpolation(b,row)?;
        let actual_sampling=if im==naga::Interpolation::Flat {None}else{Some(sample)};
        if c[0].interpolation!=Some(im) || c[0].sampling!=actual_sampling {return Err(bad("source/report/WGSL interpolation differs"))}
    }
    Ok(())
}
use crate::spirv_interface883 as spirv;
fn spirv_words(p:&PreparedShader)->Result<&[u32]> {
    match p.payload() {
        crate::payload104::ShaderPayload104::ControlledSpirv{words,..}|crate::payload104::ShaderPayload104::AuthoredSpirv{words}=>Ok(words),
        crate::payload104::ShaderPayload104::Wgsl{..}=>Err(bad("SPIR-V linkage over a WGSL payload")),
    }
}
type SourceLocations=(BTreeMap<(String,u32),u32>,BTreeSet<u32>,BTreeMap<u32,(spirv::Component,bool)>);
/// The location of every source input of a pixel program as the payload writer allocates it
/// (`iridia::program::io::entry_io`): in interface order, one location per non-system input,
/// render-target-array and viewport index; other system values are built-ins. Returns the
/// semantic locations, every allocated location and the expected built-ins (type, used).
pub(crate) fn source_input_locations(inputs:&[serde_json::Value])->Result<SourceLocations> {
    let mut by_semantic=BTreeMap::new();let mut allocated=BTreeSet::new();let mut builtins=BTreeMap::new();let mut next=0u32;
    for v in inputs {
        let used=v["used"].as_bool().ok_or_else(||bad("source input usage"))?;
        match v["system"].as_str() {
            None if v["system"].is_null()=>{
                let name=v["label"]["name"].as_str().ok_or_else(||bad("source input label"))?.to_ascii_uppercase();
                let index=u32::try_from(v["label"]["index"].as_u64().ok_or_else(||bad("source input index"))?).map_err(|_|bad("source input index"))?;
                if by_semantic.insert((name,index),next).is_some(){return Err(bad("duplicate source input semantic"))}
                allocated.insert(next);next+=1;
            },
            Some("render_target_array_index"|"viewport_array_index")=>{allocated.insert(next);next+=1;},
            Some(system)=>match spirv::fragment_builtin(system) {
                Some((b,c))=>{if builtins.insert(b,(c,used)).is_some(){return Err(bad("duplicate source system input"))}},
                None if !used=>{},
                None=>return Err(bad(format!("source system input {system} has no reflected SPIR-V built-in"))),
            },
            None=>return Err(bad("source input system value")),
        }
    }
    Ok((by_semantic,allocated,builtins))
}
/// Stage linkage on the SPIR-V words themselves: locations, built-ins, component types and
/// interpolation decorations. A semantic is tied to its location by the writer's allocation
/// over the prepared source interface, never by a debug name.
fn spirv_linkage(vertex:&PreparedShader,fragment:&PreparedShader,rows:&Rows<'_>)->Result<()> {
    let v=spirv::reflect(spirv_words(vertex)?).map_err(bad)?;
    let f=spirv::reflect(spirv_words(fragment)?).map_err(bad)?;
    spirv_interface_linkage(&v,&f,fragment.manifest()["prepared_interface"]["inputs"].as_array().ok_or_else(||bad("prepared fragment interface inputs"))?,rows)
}
pub(crate) fn spirv_interface_linkage(v:&spirv::Reflected,f:&spirv::Reflected,inputs:&[serde_json::Value],rows:&Rows<'_>)->Result<()> {
    if v.model!=spirv::ExecutionModel::Vertex||f.model!=spirv::ExecutionModel::Fragment{return Err(bad("SPIR-V entry execution models differ from the stage pair"))}
    let f4=spirv::Component{kind:spirv::Kind::Float,width:32,lanes:4};
    if v.interface.builtin_outputs.get(&spirv::BUILTIN_POSITION)!=Some(&f4){return Err(bad("SPIR-V vertex position output"))}
    // Every fragment input location is fed by a vertex output of the same type and decorations.
    for (loc,c) in &f.interface.inputs {
        let a=v.interface.outputs.get(loc).ok_or_else(||bad(format!("SPIR-V fragment location {loc} has no vertex output")))?;
        if a!=c {return Err(bad(format!("actual SPIR-V stage location {loc} type/interpolation mismatch: {a:?} / {c:?}")))}
    }
    let (by_semantic,allocated,builtins)=source_input_locations(inputs)?;
    if let Some(loc)=f.interface.inputs.keys().find(|l|!allocated.contains(l)){return Err(bad(format!("SPIR-V fragment location {loc} is no source input")))}
    for (b,c) in &f.interface.builtin_inputs {
        if builtins.get(b).map(|x|&x.0)!=Some(c){return Err(bad(format!("SPIR-V fragment built-in {b} is no source system input of its type")))}
    }
    if builtins.iter().any(|(b,(_,used))|*used&&!f.interface.builtin_inputs.contains_key(b)){return Err(bad("used source system input absent from SPIR-V"))}
    for (row,b) in rows {
        let loc=*by_semantic.get(&(row.semantic.to_ascii_uppercase(),row.index)).ok_or_else(||bad("semantic has no source input location"))?;
        let c=f.interface.inputs.get(&loc).ok_or_else(||bad(format!("semantic {}{} location {loc} absent from SPIR-V",row.semantic,row.index)))?;
        let kind=match row.scalar.to_ascii_lowercase().as_str(){"float"=>spirv::Kind::Float,"uint"=>spirv::Kind::Uint,"sint"=>spirv::Kind::Sint,_=>return Err(bad("unsupported linkage scalar"))};
        if c.component!=(spirv::Component{kind,width:32,lanes:u32::from(row.lanes)}){return Err(bad("actual SPIR-V semantic type differs from report"))}
        let (im,sample)=source_interpolation(b,row)?;
        let interpolation=match im{naga::Interpolation::Perspective=>spirv::Interpolation::Smooth,naga::Interpolation::Linear=>spirv::Interpolation::NoPerspective,naga::Interpolation::Flat=>spirv::Interpolation::Flat,_=>return Err(bad("unsupported source interpolation"))};
        let sampling=match sample{naga::Sampling::Center=>spirv::Sampling::Center,naga::Sampling::Centroid=>spirv::Sampling::Centroid,naga::Sampling::Sample=>spirv::Sampling::Sample,_=>return Err(bad("unsupported source sampling"))};
        if c.interpolation!=interpolation||c.sampling!=sampling{return Err(bad("source/report/SPIR-V interpolation differs"))}
    }
    Ok(())
}
