//! Exact emitted SPIR-V event; this does not invent an imported value correspondence.
use iridia::{precision::{ProducerRole,reference},Program};
use iridia_naga as ir;
#[derive(Clone,Debug)]pub struct EmittedSample648{pub source_word:usize,pub sample_id:u32,pub sample_type_id:u32,pub result_type_id:u32,pub result_id:u32,pub coordinate_id:u32,pub lod_id:u32,pub image_descriptor:(u32,u32),pub sampler_descriptor:(u32,u32)}
pub fn join(p:&Program,report:&ir::correspondence::EmissionReport,words:&[u32])->Result<Vec<EmittedSample648>,String>{join_module(&p.module,&p.precision,report,words)}
pub fn join_module(m:&ir::Module,receipt:&iridia::precision::PrecisionReceipt,report:&ir::correspondence::EmissionReport,words:&[u32])->Result<Vec<EmittedSample648>,String>{
 if words.first()!=Some(&0x07230203){return Err("Sample648SpvHeader".into())}let mut inst=vec![];let mut at=5;while at<words.len(){let n=(words[at]>>16)as usize;if n==0||at+n>words.len(){return Err("Sample648SpvExtent".into())}inst.push((words[at]&65535,&words[at+1..at+n]));at+=n;}
 let def=|id:u32,op:u32|->Result<&[u32],String>{let rows=inst.iter().filter(|(o,w)|*o==op&&w.len()>=2&&w[1]==id).collect::<Vec<_>>();if rows.len()!=1{return Err(format!("Sample648Definition:{id}:{op}"))}Ok(rows[0].1)};
 let val=|r|->Result<u32,String>{let rows=report.values.iter().filter(|(s,_)|*s==r).collect::<Vec<_>>();if rows.len()!=1{return Err("Sample648ValueAmbiguous".into())}if let ir::correspondence::EmittedValue::Value(id)=rows[0].1{Ok(id)}else{Err("Sample648ValueMissing".into())}};
 let float=|t|inst.iter().any(|(o,w)|*o==22&&*w==[t,32]);let vector=|t,n|inst.iter().any(|(o,w)|*o==23&&w.len()==3&&w[0]==t&&float(w[1])&&w[2]==n);
 let descriptor=|id:u32|->Result<(u32,u32),String>{let load=def(id,61)?;if load.len()!=3{return Err("Sample648HandleLoad".into())}let global=load[2];let groups=inst.iter().filter(|(o,w)|*o==71&&w.len()==3&&w[..2]==[global,34]).map(|(_,w)|w[2]).collect::<Vec<_>>();let bindings=inst.iter().filter(|(o,w)|*o==71&&w.len()==3&&w[..2]==[global,33]).map(|(_,w)|w[2]).collect::<Vec<_>>();if groups.len()!=1||bindings.len()!=1{return Err("Sample648DescriptorAmbiguous".into())}Ok((groups[0],bindings[0]))};
 let mut out=vec![];for b in receipt.bindings().iter().filter(|b|b.role==ProducerRole::SampleExplicitLod648){
  let r=receipt.records().iter().find(|r|r.word==b.source_word).ok_or("Sample648SourceMissing")?;iridia::precision::sample_source648(r)?;
  let f=match b.producer.owner{iridia::precision::FunctionOwner::EntryPoint{index,..}=>&m.entry_points[index].function,iridia::precision::FunctionOwner::Function(h)=>&m.functions[h]};
  if !iridia::precision::check_sample648(receipt,m,f,b,r)?{return Err("Sample648SourceBinding".into())}
  let ir::Expression::AccessIndex{base:sample,index:0}=f.expressions[b.producer.expression]else{return Err("Sample648Selection".into())};let mut sr=reference(&b.producer);sr.expression=sample;
  let sample_id=val(sr)?;let result_id=val(reference(&b.producer))?;let coordinate_id=val(reference(&b.operands[0]))?;let lod_id=val(reference(&b.operands[1]))?;
  let s=def(sample_id,88)?;if s.len()!=6||!vector(s[0],4)||s[3..]!=[coordinate_id,2,lod_id]{return Err("Sample648ExplicitLodOperands".into())}
  let c=def(lod_id,43)?;if c.len()!=3||!float(c[0])||c[2]!=0{return Err("Sample648LodNotPositiveZero".into())}
  let extract=def(result_id,81)?;if extract.len()!=4||!float(extract[0])||extract[2..]!=[sample_id,0]{return Err("Sample648ResultLane".into())}
  let pair=def(s[2],86)?;if pair.len()!=4{return Err("Sample648ImageSamplerPair".into())}let image_descriptor=descriptor(pair[2])?;let sampler_descriptor=descriptor(pair[3])?;
  let image_load=def(pair[2],61)?;let sampler_load=def(pair[3],61)?;
  if !inst.iter().any(|(o,w)|*o==25&&w.len()==8&&w[0]==image_load[0]&&float(w[1])&&w[2..]==[1,0,0,0,1,0])||!inst.iter().any(|(o,w)|*o==26&&*w==[sampler_load[0]]){return Err("Sample648EmittedResourceType".into())}
  let coords=inst.iter().filter(|(o,w)|matches!(*o,61|79|80|124)&&w.len()>=2&&w[1]==coordinate_id).collect::<Vec<_>>();if coords.len()!=1||!vector(coords[0].1[0],2){return Err("Sample648EmittedCoordinateType".into())}
  let ir::Expression::ImageSample{image,sampler,..}=f.expressions[sample]else{unreachable!()};let ir::Expression::GlobalVariable(ig)=f.expressions[image]else{unreachable!()};let ir::Expression::GlobalVariable(sg)=f.expressions[sampler]else{unreachable!()};
  let ib=m.global_variables[ig].binding.as_ref().ok_or("Sample648ImageBinding")?;let sb=m.global_variables[sg].binding.as_ref().ok_or("Sample648SamplerBinding")?;if image_descriptor!=(ib.group,ib.binding)||sampler_descriptor!=(sb.group,sb.binding){return Err("Sample648DescriptorMismatch".into())}
  out.push(EmittedSample648{source_word:b.source_word,sample_id,sample_type_id:s[0],result_type_id:extract[0],result_id,coordinate_id,lod_id,image_descriptor,sampler_descriptor});
 }Ok(out)
}
