//! Exact emitted uint graph for the source-bound raw-load family; no float waiver.
use iridia::precision::{ProducerRole, reference};
use iridia_naga as ir;
#[derive(Clone,Debug)]
pub struct JoinedRaw582 {
    pub source_word:usize, pub result_id:u32, pub byte_offset_id:u32,
    pub component_ids:Vec<u32>, pub descriptor:(u32,u32),
    pub imported_root:naga::Handle<naga::GlobalVariable>,
}
pub fn join(source:&ir::Module,receipt:&iridia::precision::PrecisionReceipt,report:&ir::correspondence::EmissionReport,words:&[u32],imported:&naga::Module)->Result<Vec<JoinedRaw582>,String>{
    let loads=receipt.bindings().iter().filter(|b|b.role==ProducerRole::RawLoad582).collect::<Vec<_>>();
    if loads.is_empty(){return Ok(vec![])}
    receipt.check_raw_declaration582()?;
    let roots=source.global_variables.iter().filter(|(_,g)|g.name.as_deref()==Some("sb4")).collect::<Vec<_>>();
    if roots.len()!=1{return Err("Raw582CarrierSourceRootAmbiguous".into())}
    let binding=roots[0].1.binding.as_ref().ok_or("Raw582CarrierSourceBinding")?;
    let target=imported.global_variables.iter().filter(|(_,g)|g.binding.as_ref().is_some_and(|b|b.group==binding.group && b.binding==binding.binding)).collect::<Vec<_>>();
    if target.len()!=1 || target[0].1.space!=(naga::AddressSpace::Storage{access:naga::StorageAccess::LOAD}) {return Err("Raw582CarrierImportedRoot".into())}
    let mut inst=vec![];let mut at=5;
    if words.first()!=Some(&0x07230203){return Err("Raw582CarrierHeader".into())}
    while at<words.len(){let n=(words[at]>>16)as usize;if n==0 || at+n>words.len(){return Err("Raw582CarrierExtent".into())}inst.push((words[at]&65535,&words[at+1..at+n]));at+=n;}
    let value=|r|->Result<u32,String>{let rows=report.values.iter().filter(|(s,_)|*s==r).collect::<Vec<_>>();if rows.len()!=1{return Err("Raw582CarrierValueAmbiguous".into())}match rows[0].1{ir::correspondence::EmittedValue::Value(id)=>Ok(id),_=>Err("Raw582CarrierValueMissing".into())}};
    let def=|id:u32,opcode:u32|->Result<&[u32],String>{let rows=inst.iter().filter(|(op,w)|*op==opcode && w.len()>=2 && w[1]==id).collect::<Vec<_>>();if rows.len()!=1{return Err(format!("Raw582CarrierDef:{id}:{opcode}"))}Ok(rows[0].1)};
    let u32_type=|ty|inst.iter().any(|(op,w)|*op==21 && *w==[ty,32,0]);
    let integer=|id:u32,v:u32|{inst.iter().any(|(op,w)|*op==43 && w.len()==3 && w[1..]==[id,v] && inst.iter().any(|(op,t)|*op==21 && t.len()==3 && t[0]==w[0] && t[1]==32))};
    let candidates=inst.iter().filter(|(op,w)|*op==59 && w.len()>=3 && inst.iter().any(|(op,d)|*op==71 && *d==[w[1],34,binding.group]) && inst.iter().any(|(op,d)|*op==71 && *d==[w[1],33,binding.binding])).collect::<Vec<_>>();
    if candidates.len()!=1{return Err("Raw582CarrierDescriptorMissingOrAmbiguous".into())}let global=candidates[0].1[1];
    let mut joined=vec![];
    for b in loads {
        let id=value(reference(&b.producer))?;let byte=value(reference(&b.operands[0]))?;
        let component_ids=if b.width.get()==1 {
            let r=receipt.records().iter().find(|r|r.word==b.source_word).ok_or("Raw644SourceMissing")?;
            iridia::precision::raw_load_source582(r)?;
            if !iridia::precision::raw_scalar582(r){return Err("Raw644ScalarSourceUnsupported".into())}
            vec![id]
        }else{
            let composite=def(id,80)?;
            if composite.len()!=6 || !inst.iter().any(|(op,w)|*op==23 && w.len()==3 && w[0]==composite[0] && u32_type(w[1]) && w[2]==4){return Err("Raw582CarrierResultType".into())}
            composite[2..].to_vec()
        };
        for (component,&id) in component_ids.iter().enumerate(){
            let select=def(id,169)?;if select.len()!=5 || !u32_type(select[0]) || !integer(select[4],0){return Err("Raw582CarrierZeroArm".into())}
            let compare=def(select[2],176)?;if compare.len()!=4 || !inst.iter().any(|(op,w)|*op==20 && *w==[compare[0]]){return Err("Raw582CarrierBoundsCompare".into())}
            let length=def(compare[3],68)?;if length.len()!=4 || !u32_type(length[0]) || length[2..]!=[global,0]{return Err("Raw582CarrierLengthResource".into())}
            let index=def(compare[2],128)?;if index.len()!=4 || !u32_type(index[0]) || !integer(index[3],component as u32){return Err("Raw582CarrierComponentAddress".into())}
            let word=def(index[2],194)?;if word.len()!=4 || !u32_type(word[0]) || word[2]!=byte || !integer(word[3],2){return Err("Raw582CarrierByteAddress".into())}
            let read=def(select[3],61)?;if read.len()!=3 || !u32_type(read[0]){return Err("Raw582CarrierLoadType".into())}
            let access=def(read[2],65)?;
            let safe_id=if access.len()==5 && access[2]==global && integer(access[3],0){access[4]}
                else if access.len()==4 {let member=def(access[2],65)?;if member.len()!=4 || member[2]!=global || !integer(member[3],0){return Err("Raw582CarrierLoadResource".into())}access[3]}
                else{return Err("Raw582CarrierLoadResource".into())};
            let safe=def(safe_id,169)?;if safe.len()!=5 || !u32_type(safe[0]) || safe[2]!=select[2] || safe[3]!=compare[2] || !integer(safe[4],0){return Err("Raw582CarrierSafeAddress".into())}
        }
        joined.push(JoinedRaw582{source_word:b.source_word,result_id:id,byte_offset_id:byte,component_ids,descriptor:(binding.group,binding.binding),imported_root:target[0].0});
    }
    Ok(joined)
}
pub fn roots(joined:&[JoinedRaw582])->Vec<naga::Handle<naga::GlobalVariable>>{let mut out=vec![];for j in joined{if !out.contains(&j.imported_root){out.push(j.imported_root)}}out}
