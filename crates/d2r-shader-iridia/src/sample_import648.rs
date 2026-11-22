//! Exact source -> emitted sample -> importer result -> protected use. No sample evaluation.
use iridia::precision::{ProducerRole,reference};
use iridia_naga as ir;
#[path = "../../../.patched/iridia-0.5.0-3cbe8f570937/src/precision/exact_eval.rs"]
#[allow(dead_code)]mod imported_eval;
#[derive(Clone,Debug)]pub struct JoinedSampleUse648{pub consumer_word:usize,pub source_operand:usize,pub source_lane:u8,pub consumer:naga::correspondence::ExpressionRef,pub operand:(u32,naga::correspondence::ExpressionRef),pub lane:u8}
#[derive(Clone,Debug)]pub struct JoinedSample648{pub source:ir::correspondence::ExpressionRef,pub emitted:crate::sample_emit648::EmittedSample648,pub imported:naga::correspondence::ExpressionRef,pub sample:naga::correspondence::ExpressionRef,pub uses:Vec<JoinedSampleUse648>,pub analysis:imported_eval::Analysis}
pub fn results_for(samples:&[JoinedSample648],owner:naga::correspondence::Owner)->Vec<naga::Handle<naga::Expression>>{samples.iter().filter(|s|s.imported.owner==owner).map(|s|s.imported.expression).collect()}
pub fn join(source:&ir::Module,receipt:&iridia::precision::PrecisionReceipt,emitted:&ir::correspondence::EmissionReport,words:&[u32],imported:&naga::Module,report:&naga::correspondence::ImportReport,binary:&[crate::precision_carrier::JoinedProducer],raw_roots:&[naga::Handle<naga::GlobalVariable>])->Result<Vec<JoinedSample648>,String>{
 let events=crate::sample_emit648::join_module(source,receipt,emitted,words)?;if events.is_empty(){return Ok(vec![])}if report.sample_revision!=1{return Err("Sample648ImportReportRevisionUnsupported".into())}
 let mut out=vec![];for event in events{
  let b=receipt.bindings().iter().find(|b|b.role==ProducerRole::SampleExplicitLod648&&b.source_word==event.source_word).ok_or("Sample648SourceBindingMissing")?;
  let rows=report.samples648.iter().filter(|r|match r{naga::correspondence::ImportedSample648::Direct{result_id,..}|naga::correspondence::ImportedSample648::Unsupported{result_id,..}=>*result_id==event.result_id}).collect::<Vec<_>>();if rows.len()!=1{return Err("Sample648ImportMissingOrAmbiguous".into())}
  let naga::correspondence::ImportedSample648::Direct{sample_id,sample_type_id,result_id:_,type_id,sample,producer,coordinate,lod,coordinate_expression,lod_expression,..}=rows[0]else{return Err("Sample648ImportNotDirect".into())};
  if *sample_id!=event.sample_id||*sample_type_id!=event.sample_type_id||*type_id!=event.result_type_id||coordinate.0!=event.coordinate_id||lod.0!=event.lod_id||sample.owner!=producer.owner||coordinate.1.owner!=producer.owner||lod.1.owner!=producer.owner{return Err("Sample648ImportIdsOrOwnerMismatch".into())}
  let f=crate::unary_carrier::function(imported,*producer);let (actual_sample,actual_coordinate,actual_lod)=imported_eval::sample_shape648(imported,f,producer.expression)?;
  if actual_sample!=sample.expression||actual_coordinate!=coordinate.1.expression||actual_lod!=lod.1.expression{return Err("Sample648ImportExpressionMismatch".into())}
  if &f.expressions[actual_coordinate]!=coordinate_expression||&f.expressions[actual_lod]!=lod_expression{return Err("Sample648ImportArgumentChanged".into())}
  let naga::Expression::ImageSample{image,sampler,..}=f.expressions[sample.expression]else{return Err("Sample648ImportSampleMissing".into())};let naga::Expression::GlobalVariable(ig)=f.expressions[image]else{return Err("Sample648ImportImageRoot".into())};let naga::Expression::GlobalVariable(sg)=f.expressions[sampler]else{return Err("Sample648ImportSamplerRoot".into())};
  let ib=imported.global_variables[ig].binding.as_ref().ok_or("Sample648ImportImageBinding")?;let sb=imported.global_variables[sg].binding.as_ref().ok_or("Sample648ImportSamplerBinding")?;if (ib.group,ib.binding)!=event.image_descriptor||(sb.group,sb.binding)!=event.sampler_descriptor{return Err("Sample648ImportDescriptorMismatch".into())}
  if f.expression_controls.contains_key(&producer.expression)||f.expression_controls.contains_key(&sample.expression){return Err("Sample648UnexpectedSampleArithmeticControl".into())}
  let p=receipt.unary_proofs().iter().find(|p|p.producer==b.producer&&p.source_word==b.source_word).ok_or("Sample648SourceUseProofMissing")?;
  let analysis=imported_eval::analyze_imported_sampled648(imported,producer.owner,&[],&[producer.expression],raw_roots,&[producer.expression])?;
  let actual=analysis.uses.iter().filter(|u|u.atom.opaque.is_some()).collect::<Vec<_>>();let mut matched=vec![];let mut uses=vec![];
  for u in &p.uses{let c=binary.iter().find(|c|c.source.owner==b.producer.owner&&c.source.expression==u.edge.producer&&c.controlled).ok_or("Sample648ControlledConsumerMissing")?;if c.imported.owner!=producer.owner||f.expression_controls.get(&c.imported.expression)!=Some(&naga::ArithmeticControl::NoContraction){return Err("Sample648ConsumerControlOrOwner".into())}let a=c.operands.get(u.edge.ordinal).ok_or("Sample648ConsumerOrdinal")?;
   let indices=actual.iter().enumerate().filter(|(_,v)|v.producer==c.imported.expression&&v.operand==a.1.expression&&v.ordinal==u.edge.ordinal&&v.lane==u.edge.lane&&v.atom.opaque==Some(producer.expression)&&v.atom.bits.is_none()&&v.atom.origin.is_none()).map(|(i,_)|i).collect::<Vec<_>>();if indices.len()!=1||matched.contains(&indices[0]){return Err("Sample648ImportedUseMismatch".into())}matched.push(indices[0]);uses.push(JoinedSampleUse648{consumer_word:u.source_word,source_operand:u.source_operand,source_lane:u.source_lane,consumer:c.imported,operand:*a,lane:u.edge.lane});
  }
  if actual.len()!=matched.len(){return Err("Sample648UnjoinedImportedUse".into())}
  out.push(JoinedSample648{source:reference(&b.producer),emitted:event,imported:*producer,sample:*sample,uses,analysis});
 }Ok(out)
}
