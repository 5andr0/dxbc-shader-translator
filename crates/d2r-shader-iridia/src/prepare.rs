use crate::{CacheIdentity, Error, PreparedShader, Result, SourceIdentity, SourceShader, Stage, lift};
use crate::resource_contract::{CallerContract, apply_resources, prepare_contract};
use crate::stage_link::{DrawContract, LinkManifest, apply_draw_contract};
use serde::{Deserialize,Serialize};
use serde_json::json;
use std::collections::BTreeMap;

pub const PAIR_SCHEMA:&str="d2r-iridia50-linked-pair-v1";
pub const ICE_PROFILE:&str="ice-bolt-original-mesh-snapshot-profile1";

/// Explicit limits of the eventual target. This input is part of the cache
/// identity; only the consumer/device can prove its reported limits are actual.
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetContract {
    pub revision:String,
    pub backend:String,
    pub max_bind_groups:u32,
    pub max_bindings_per_bind_group:u32,
    pub max_uniform_buffer_binding_size:u64,
    pub max_storage_buffer_binding_size:u64,
    #[serde(default)]
    pub compute:Option<ComputeLimits>,
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeLimits {pub max_invocations_per_workgroup:u32,pub max_workgroup_size:[u32;3],pub max_workgroup_storage_size:u64}
impl TargetContract {
    pub(crate) fn validate(&self)->Result<()> {
        if self.revision!="d2r-iridia50-target-v1" || !matches!(self.backend.as_str(),"dx12"|"vulkan"|"metal")
            || self.max_bind_groups==0 || self.max_bind_groups>8 || self.max_bindings_per_bind_group==0
            || self.max_bindings_per_bind_group>65536 || self.max_uniform_buffer_binding_size==0
            || self.max_storage_buffer_binding_size==0 { return Err(Error::Unsupported("target contract/reported limits".into())); }
        Ok(())
    }
    pub(crate) fn resources(&self,c:&CallerContract)->Result<()> {
        for r in &c.resources {
            if r.group>=self.max_bind_groups || r.binding>=self.max_bindings_per_bind_group { return Err(Error::Unsupported("resource destination exceeds target limits".into())); }
            use crate::resource_contract::ResourceContractKind as K;
            match &r.resource {
                K::ConstantBuffer{bound_bytes} if u64::from(*bound_bytes)>self.max_uniform_buffer_binding_size=>return Err(Error::Unsupported("CB bound extent exceeds target limit".into())),
                K::RawBuffer{view}|K::StructuredBuffer{view,..}|K::TypedBuffer{view,..}|K::StorageBuffer{view,..}=>{
                    let range=view.range().map_err(Error::Invalid)?;
                    if range.end-range.start>self.max_storage_buffer_binding_size { return Err(Error::Unsupported("storage view exceeds target limit".into())); }
                },
                _=>{},
            }
        }
        Ok(())
    }
    pub(crate) fn compute_module(&self,m:&iridia_naga::Module)->Result<serde_json::Value> {
        let c=self.compute.as_ref().ok_or_else(||Error::Unsupported("compute target limits must be supplied explicitly".into()))?;
        if c.max_invocations_per_workgroup==0 || c.max_workgroup_size.iter().any(|s|*s==0) || c.max_workgroup_storage_size==0 {return Err(Error::Invalid("zero compute target limit".into()))}
        if m.entry_points.len()!=1 || m.entry_points[0].stage!=iridia_naga::ShaderStage::Compute{return Err(Error::Unsupported("one compute entry required".into()))}
        let s=m.entry_points[0].workgroup_size;
        let total=s.iter().try_fold(1u64,|n,v|n.checked_mul(u64::from(*v))).ok_or_else(||Error::Invalid("workgroup invocation overflow".into()))?;
        let info=iridia_naga::valid::Validator::new(iridia_naga::valid::ValidationFlags::all(),iridia_naga::valid::Capabilities::all()).validate(m).map_err(|e|Error::Validation(format!("compute target module: {e:?}")))?;
        // WebGPU compute pipeline validation counts each statically used
        // workgroup variable separately, rounded to sixteen bytes.
        let memory=m.global_variables.iter().filter(|(h,g)|g.space==iridia_naga::AddressSpace::WorkGroup&&!info.get_entry_point(0)[*h].is_empty()).try_fold(0u64,|n,(_,g)|n.checked_add(u64::from(m.types[g.ty].inner.size(m.to_ctx())).div_ceil(16)*16)).ok_or_else(||Error::Invalid("workgroup memory overflow".into()))?;
        if total>u64::from(c.max_invocations_per_workgroup)||s.iter().zip(c.max_workgroup_size).any(|(v,a)|*v>a)||memory>c.max_workgroup_storage_size{return Err(Error::Unsupported("actual compute program exceeds supplied target limits".into()))}
        Ok(json!({"workgroup_size":s,"invocations":total,"workgroup_memory_bytes":memory,"required_limits_checked":true,"actual_device_attested":false}))
    }
}

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedPair {
    pub schema:String,
    pub profile:String,
    pub vertex:PreparedShader,
    pub fragment:PreparedShader,
    pub linkage:LinkManifest,
}
impl PreparedPair {
    pub fn validate_artifact(&self)->Result<()> {
        if self.schema!=PAIR_SCHEMA || self.profile!=ICE_PROFILE { return Err(Error::Unsupported("unknown linked profile/schema".into())); }
        validate_ice_identity(&self.vertex.identity().source,Stage::Vertex)?;
        validate_ice_identity(&self.fragment.identity().source,Stage::Fragment)?;
        self.vertex.validate_artifact()?;self.fragment.validate_artifact()?;
        if self.vertex.target_capabilities()!=self.fragment.target_capabilities(){return Err(Error::Identity("paired target contracts differ".into()))}
        let linkage=serde_json::to_value(&self.linkage)?;
        if self.vertex.linked_interface()!=&linkage || self.fragment.linked_interface()!=&linkage
            || self.linkage.padding_allowed || !self.linkage.emitted_notes.is_empty() {
            return Err(Error::Identity("linked pair report mismatch or approximation".into()));
        }
        crate::validation::validate_pair_linkage(self)?;
        Ok(())
    }
}

fn validate_ice_identity(s:&SourceIdentity,stage:Stage)->Result<()> {
    let (path,resource,bytes,offset,count,dxbc)=match stage {
        Stage::Vertex=>("data/hd/shaders/vs_popcorn_uber_mesh_v.bin","f0d42cec62e831f91be554efec73dd5b1cca5ddea8d51b0a57f9e94f724c8c88",11160780,8352384,3868,"2babfc57080b5033e58490aef43f90eb1952207599a55b2353b864f07b237cc0"),
        Stage::Fragment=>("data/hd/shaders/ps_popcorn_uber_mesh_p.bin","e5a8b7faba59e4adcc3f7fba58afc9cd8a6877853bee51cc8572aea30c323a2b",26001380,19259764,4560,"cb1a5b1b09ec5059f94b74173781dfff6b1092ccae8a872c09026c40696037bd"),
        _=>return Err(Error::Unsupported("Ice profile stage".into())),
    };
    if s.stage!=stage || s.path!=path || s.resource_sha256!=resource || s.resource_bytes!=bytes
        || s.offset!=offset || s.byte_count!=count || s.dxbc_sha256!=dxbc {
        return Err(Error::Identity("unknown/mismatched registered Ice source profile".into()));
    }
    Ok(())
}

pub(crate) fn bound_program(source:&SourceShader,contract:&CallerContract)->Result<(iridia::Program,serde_json::Value)> {
    let declaration=lift(source)?;
    let sizes=prepare_contract(&declaration,contract).map_err(Error::Unsupported)?;
    let mut program=iridia_dxbc::lift_with(source.bytes(),&sizes).map_err(|e|Error::Unsupported(format!("bound lift: {e}")))?;
    program.precision.require_unmapped_transform("adapter preparation").map_err(Error::Unsupported)?;
    let sampling=crate::apply_sampling(&mut program)?;
    Ok((program,sampling))
}
/// Explicit controlled preparation, separate from the unchanged ordinary preparation gate.
/// Every transform below transports and validates the source receipt; original
/// catalogue/numerical/current-host admission remains in the renderer.
pub(crate) fn bound_controlled_program104(source:&SourceShader,contract:&CallerContract)->Result<(iridia::Program,serde_json::Value)> {
    let declaration=lift(source)?;
    let sizes=prepare_contract(&declaration,contract).map_err(Error::Unsupported)?;
    let mut program=iridia_dxbc::lift_with(source.bytes(),&sizes).map_err(|e|Error::Unsupported(format!("bound controlled lift: {e}")))?;
    let expected=format!("sha256:{}",source.identity().dxbc_sha256);
    if program.provenance.source_hash.as_deref()!=Some(expected.as_str()) {return Err(Error::Identity("controlled bound source provenance".into()))}
    program.precision.validate(&program.module,Some(&expected)).map_err(Error::Unsupported)?;
    let sampling=crate::apply_sampling(&mut program)?;
    program.precision.validate(&program.module,Some(&expected)).map_err(Error::Unsupported)?;
    Ok((program,sampling))
}
pub(crate) fn source_declarations(p:&iridia::Program)->Result<serde_json::Value> {
    let mut out=Vec::new();
    for r in &p.interface.resources {
        let json=serde_json::to_string(r)?;
        out.push(json!({"resource_id":r.id.0,"original_serialized_declaration":json,"sha256":crate::sha256(json.as_bytes())}));
    }
    Ok(json!(out))
}

/// First selected production candidate. Additional families must be registered
/// against their own source/caller contracts; generic CPU success is insufficient.
pub fn prepare_ice_pair(vs:&SourceShader,ps:&SourceShader,vertex:&CallerContract,fragment:&CallerContract,draw:&DrawContract,target:&TargetContract)->Result<PreparedPair> {
    validate_ice_identity(vs.identity(),Stage::Vertex)?;validate_ice_identity(ps.identity(),Stage::Fragment)?;
    target.validate()?;target.resources(vertex)?;target.resources(fragment)?;
    if draw.base_group!=0 || draw.base_binding!=127 || draw.base_group>=target.max_bind_groups || draw.base_binding>=target.max_bindings_per_bind_group || target.max_uniform_buffer_binding_size<16 {
        return Err(Error::Unsupported("exact Ice first-instance transport requires group0/binding127/16B".into()));
    }
    let (mut v,vsampling)=bound_program(vs,vertex)?;let (mut p,psampling)=bound_program(ps,fragment)?;
    let vdecl=source_declarations(&v)?;let pdecl=source_declarations(&p)?;
    let original_vertex_interface=serde_json::to_value(&v.interface)?;let original_fragment_interface=serde_json::to_value(&p.interface)?;
    let vb=apply_resources(&mut v,vertex).map_err(Error::Unsupported)?;let pb=apply_resources(&mut p,fragment).map_err(Error::Unsupported)?;
    if !v.notes.is_empty() || !p.notes.is_empty() {return Err(Error::Unsupported(format!("unresolved source notes after resource adaptation: {:?}/{:?}",v.notes,p.notes)))}
    let draw_manifest=apply_draw_contract(&mut v,draw).map_err(Error::Unsupported)?;
    let linked=crate::stage_link::link_payload_pair104(&v,&p,&draw.topology,false).map_err(Error::Unsupported)?;
    let target_json=serde_json::to_value(target)?;let link_json=serde_json::to_value(&linked.manifest)?;
    let vc=json!({"profile":ICE_PROFILE,"resources":vertex,"draw":draw});let pc=json!({"profile":ICE_PROFILE,"resources":fragment,"draw":null});
    let passes=BTreeMap::from([(crate::resource_contract::RESOURCE_REVISION.to_string(),2),(crate::stage_link::LINK_REVISION.to_string(),1),(crate::stage_link::DRAW_REVISION.to_string(),1),(crate::sample_lod::SAMPLE_LOD_REVISION.to_string(),2)]);
    let vm=json!({"stage":"vertex","entry_point":v.entry,"original_interface":original_vertex_interface,"prepared_interface":v.interface,"source_declarations":vdecl,"bindings":vb,"draw":draw_manifest,"sampling":vsampling,"outstanding_notes":v.notes,"stock_naga_validated":true,"gpu_verified":false});
    let pm=json!({"stage":"fragment","entry_point":p.entry,"original_interface":original_fragment_interface,"prepared_interface":p.interface,"source_declarations":pdecl,"bindings":pb,"draw":null,"sampling":psampling,"outstanding_notes":p.notes,"stock_naga_validated":true,"gpu_verified":false});
    let vid=CacheIdentity::current(vs.identity().clone(),&vc,&link_json,&target_json,&vm,passes.clone())?;
    let pid=CacheIdentity::current(ps.identity().clone(),&pc,&link_json,&target_json,&pm,passes)?;
    let out=PreparedPair {schema:PAIR_SCHEMA.into(),profile:ICE_PROFILE.into(),
        vertex:PreparedShader::checked_payload(vid,vc,link_json.clone(),target_json.clone(),vm,linked.vertex)?,
        fragment:PreparedShader::checked_payload(pid,pc,link_json,target_json,pm,linked.fragment)?,linkage:linked.manifest};
    out.validate_artifact()?;Ok(out)
}

