//! Authored adapter policy. Original and host inputs are supplied at run time and
//! authenticated; none are stored in the repository.
//! Exact shared630 compile-time Water admission. Never a current-frame permit.
use super::{CallerCapability, CallerEvidence, NullImageFixture, CALLER_REVISION};
use crate::{Error, Result, SourceIdentity, Stage, PreparedShader};
use crate::resource_contract::{CallerContract, RegisterClass as C, ResourceContractKind as K, RESOURCE_REVISION};
use serde_json::{json, Value};
#[path="water_policy649_cpu.rs"] mod cpu;
pub use cpu::{PROFILE, RULE, CONTRACT};

/// Sealed only after hashing the actual borrowed source contract/header/bank and
/// current630 light exports. It cannot be deserialized or made from a SHA label.
/// The host renderer owns the upload; this capability only authenticates this
/// immutable compile-time domain, never GPU residency.
#[derive(Clone)]
pub struct WaterSource649 { evidence: Value }
impl WaterSource649 {
    #[allow(clippy::too_many_arguments)]
    pub fn seal(contract: &[u8], original_null_source45: &[u8], bank: &[u8],
        counts: &[u8], indices: &[u8], records: &[u8], unit: &[u8]) -> Result<Self> {
        let bytes=[contract,original_null_source45,bank,counts,indices,records,unit];
        let hashes=bytes.map(crate::sha256);
        let pins=std::array::from_fn(|i|cpu::Pin{bytes:bytes[i].len(),sha:&hashes[i]});
        cpu::inputs(&pins).map_err(Error::Identity)?;
        cpu::grid(counts,indices,records,unit).map_err(Error::Identity)?;
        // The entire pinned contract authenticates all original texture metadata,
        // including undeclared t35/t36; do not infer an image from its absence.
        Ok(Self{evidence:receipt()})
    }
    pub fn evidence(&self) -> &Value { &self.evidence }
}
fn receipt() -> Value {
    json!({"revision":RULE,"profile":PROFILE,"shared630_contract_sha256":CONTRACT,
        "inputs":cpu::INPUTS.map(|(bytes,sha)|json!({"bytes":bytes,"sha256":sha})),
        "original_vertex_sha256":cpu::VS,"original_fragment_sha256":cpu::PS,
        "source_null_image_slots":cpu::NULLS,"light_view":[0,19,16,304],
        "original_grid":"one cell; packed01000140;0,3,6,9,12,15 then zero;5point/1unit",
        "t35_t36":"not declared by selected original PS; full source mip custody remains Water645 obligation",
        "grants_current_host_authority":false})
}
fn fixtures() -> Vec<NullImageFixture> {
    cpu::NULLS.into_iter().map(|s|NullImageFixture{
        profile:PROFILE.into(),original_dxbc_sha256:cpu::PS.into(),group:1,binding:32+s,
        original_register:format!("ps:t{s}"),api:"D3D12-descriptor-table-null-SRV".into(),resource_is_null:true,
        dimension:match s{33=>"TextureCube",39|40=>"Texture2DArray",44=>"Texture2D",_=>"Texture3D"}.into(),
        format:"R32G32B32A32_FLOAT".into(),array_layers:if matches!(s,39|40){1}else{0},mip_levels:1,
        first_array_slice:0,most_detailed_mip:0,shader_component_mapping:5768,plane_slice:0,resource_min_lod_clamp_bits:0,
        accepted_host_provenance_sha256:CONTRACT.into(),rule_revision:RULE.into(),
    }).collect()
}
impl CallerCapability {
    pub fn seal_water649(source: &WaterSource649) -> Result<Self> {
        if source.evidence!=receipt(){return Err(Error::Identity("Water649 sealed source differs".into()));}
        let images=fixtures();
        for f in &images {super::validate_fixture(PROFILE,cpu::PS,f).map_err(Error::Identity)?;}
        Ok(Self{evidence:CallerEvidence{revision:CALLER_REVISION.into(),profile:PROFILE.into(),
            original_dxbc_sha256:cpu::PS.into(),fixtures:images,unit_consumed_t4:None,
            null_buffers:vec![],grants_current_host_authority:false},water649:Some(source.clone())})
    }
}
pub(crate) fn check_cap(cap: &CallerCapability) -> Result<&Value> {
    let source=cap.water649.as_ref().ok_or_else(||Error::Identity("Water649 requires actual shared630 source seal".into()))?;
    check_evidence(&cap.evidence)?;
    if source.evidence!=receipt(){return Err(Error::Identity("Water649 source provenance receipt differs".into()));}
    Ok(&source.evidence)
}
fn check_evidence(e: &CallerEvidence) -> Result<()> {
    let slots=e.fixtures.iter().map(|f|f.binding.checked_sub(32).ok_or_else(||Error::Identity("Water649 null namespace".into()))).collect::<Result<Vec<_>>>()?;
    cpu::null_slots(&slots).map_err(Error::Identity)?;
    if e.revision!=CALLER_REVISION||e.profile!=PROFILE||e.original_dxbc_sha256!=cpu::PS||
        e.fixtures!=fixtures()||e.unit_consumed_t4.is_some()||!e.null_buffers.is_empty()||e.grants_current_host_authority {
        return Err(Error::Identity("Water649 exact source-null images required; live lights/t43 cannot project".into()));
    }
    Ok(())
}
pub(crate) fn check_source(s: &SourceIdentity) -> Result<()> {
    let stage=match s.stage{Stage::Vertex=>0,Stage::Fragment=>1,_=>2};
    cpu::stage_source(stage,&s.path,&s.resource_sha256,s.resource_bytes,s.offset,s.byte_count,&s.dxbc_sha256).map_err(Error::Identity)
}
/// Whole original declaration join. No extra/missing rows or typed/null buffers.
/// Projection may omit ONLY the eight images and their unused sampler dependencies.
pub(crate) fn check_contract(stage: Stage, c: &CallerContract, projected: bool) -> Result<()> {
    let group=match stage{Stage::Vertex=>0,Stage::Fragment=>1,_=>return Err(Error::Identity("Water649 caller stage".into()))};
    let cb: &[u32]=if group==0{&[0,3,4,10]}else{&[0,1,2,3,4,6,10]};
    let tex: &[u32]=if group==0{&[10,11]}else{&[10,11,12,13,33,34,37,38,39,40,43,44,45,46,47,48]};
    let sam: &[u32]=if group==0{&[0,3]}else{&[0,1,3,4,6,7]};
    let expected=cb.len()+tex.len()+sam.len()+if group==1{3}else{0};
    if c.revision!=RESOURCE_REVISION||c.resources.len()!=expected{return Err(Error::Identity("Water649 whole source caller row count/revision".into()));}
    let mut keys=std::collections::BTreeSet::new();
    for r in &c.resources {
        let s=r.register;
        if r.space!=0||r.count!=1||r.group!=group||!keys.insert((r.class,s)) {
            return Err(Error::Identity("Water649 caller source namespace/duplicate".into()));
        }
        let binding=match r.class{C::ConstantBuffer=>Some(s),C::ShaderResource=>s.checked_add(32),C::Sampler=>s.checked_add(128),_=>None}
            .ok_or_else(||Error::Identity("Water649 invalid caller binding namespace".into()))?;
        let null=group==1&&projected&&r.class==C::ShaderResource&&cpu::NULLS.contains(&s);
        let unused_sampler=group==1&&projected&&r.class==C::Sampler&&[4,6,7].contains(&s);
        let valid=if null||unused_sampler {
            // Existing projection independently proves the source declaration
            // digest and entrypoint unreadness; this gate does not replace it.
            matches!(&r.resource,K::UnreadDeclaration{reason,..} if *reason==if null{crate::resource_contract::UnreadReason::MatchedNull}else{crate::resource_contract::UnreadReason::UnusedProjectionDependency})
        } else {match &r.resource {
            K::ConstantBuffer{bound_bytes}=>r.class==C::ConstantBuffer&&cb.contains(&s)&&*bound_bytes==1536,
            K::Texture{dimension,arrayed,multisampled,sample}=>r.class==C::ShaderResource&&tex.contains(&s)&&
                dimension==match s{33|34=>"cube",45..=48=>"d3",_=>"d2"}&&*arrayed==matches!(s,39|40)&&!*multisampled&&sample=="float",
            K::Sampler{comparison}=>r.class==C::Sampler&&sam.contains(&s)&&*comparison==(s>=6),
            K::StructuredBuffer{stride_bytes,view}=>group==1&&r.class==C::ShaderResource&&[4,5].contains(&s)&&
                *stride_bytes==if s==4{4}else{1024}&&view.source_first_element==0&&view.source_num_elements==1&&
                view.source_stride_bytes==u64::from(*stride_bytes)&&view.source_resource_bytes==u64::from(*stride_bytes),
            K::TypedBuffer{format,view}=>group==1&&r.class==C::ShaderResource&&s==8&&format=="R32G32B32A32_FLOAT"&&
                view.source_first_element==0&&view.source_num_elements==19&&view.source_stride_bytes==16&&view.source_resource_bytes==304,
            _=>false,
        }};
        if r.binding!=binding||!valid{return Err(Error::Identity(format!("Water649 exact caller row differs {group}/{s}")));}
    }
    Ok(())
}
/// Cache evidence is consistency data. It cannot recreate WaterSource649 or
/// authorize runtime payload upload; consumers retain/reseal the actual source.
pub(crate) fn check_cached_stage(p: &PreparedShader) -> Result<()> {
    if p.caller_contract()["profile"]!=PROFILE {
        if !p.caller_contract()["water649"].is_null(){return Err(Error::Identity("Water649 receipt on another profile".into()));}
        return Ok(());
    }
    check_source(&p.identity().source)?;
    if p.caller_contract()["water649"]!=receipt()||p.payload().kind()!="controlled-spirv104"||
        p.payload().source_sha256()!=Some(p.identity().source.dxbc_sha256.as_str())||p.manifest()["draw"]!=Value::Null {
        return Err(Error::Identity("Water649 controlled source/provenance/draw cache gate".into()));
    }
    check_contract(p.stage(),&p.resource_contract()?,p.stage()==Stage::Fragment)?;
    if p.stage()==Stage::Fragment {
        let r: super::CallerProjectionReport=serde_json::from_value(p.manifest()["caller_projection"].clone())?;
        check_evidence(&r.evidence)?;
        if r.unread_resources.iter().any(|r|!cpu::NULLS.contains(&r.register))||
            r.operations.iter().any(|r|!cpu::NULLS.contains(&r.source_register)) {
            return Err(Error::Identity("Water649 projection outside exact source-null images".into()));
        }
        let unique=r.operations.iter().map(|o|(&o.function,o.expression)).collect::<std::collections::BTreeSet<_>>();
        if unique.len()!=r.operations.len(){return Err(Error::Identity("Water649 duplicate projected operation".into()));}
        for slot in cpu::NULLS {
            let mut actual=[0usize;3];
            for op in r.operations.iter().filter(|o|o.source_register==slot) {
                if op.source_result_semantics!="DefinedMatchedNullZero" {
                    return Err(Error::Identity("Water649 source-null result semantics differ".into()));
                }
                actual[0]+=1;
                actual[match op.source_kind.as_str(){"exact"=>1,"comparison-zero"=>2,_=>return Err(Error::Identity("Water649 unreviewed source operation".into()))}]+=1;
            }
            let expected=match slot{39=>[50,10,40],33|40=>[4,0,4],_=>[1,1,0]};
            if actual!=expected{return Err(Error::Identity(format!("Water649 original operation coverage differs t{slot}")));}
        }
    }
    Ok(())
}

