//! Exact authored829 receiver domain. Original19 and all non-idle states remain separate.
use std::sync::Arc;use serde_json::{Value,json};use sha2::{Digest,Sha256};
#[path="policy829.rs"]mod policy;
pub const RULE:&str="authored-idle-specular830-v1";pub const POLICY:&str="de728cc8711e0e70888f258a9edd00251679451b63df3f411251a3135e031005";
fn hash(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}fn need(v:bool,s:&str)->Result<(),String>{if v{Ok(())}else{Err(s.into())}}
#[derive(Clone,Debug)]pub struct Domain{policy:Arc<[u8]>,ground:Arc<[u8]>,flying:Arc<[u8]>,states:Vec<(String,u32)>,lights:[Arc<[u8]>;3]}
impl Domain{
 pub fn seal(bytes:&[u8],ground:&[u8],flying:&[u8],states:&[(String,u32)])->Result<Self,String>{
  need(hash(bytes)==POLICY,"830 exact frozen policy829 bytes")?;let p:Value=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
  need(states.len()==4&&p["policy"]==policy::NAME&&p["source_v4"]["sha256"]=="4de6d5adf4d411724c0dd898b163f946d2ac0b0b40e86bbee8edc53e1c40a781","830 exact authored policy/base")?;
  for(b,k)in[(ground,"original_ground"),(flying,"original_flying")]{need(p[k]["slots"]["8"]["bytes"].as_u64()==Some(b.len()as u64)&&p[k]["slots"]["8"]["sha256"]==hash(b),"830 actual complete source record inputs")?;}
  let refs=std::array::from_fn(|i|(states[i].0.as_str(),states[i].1));let data=policy::construct(ground,flying,refs,true)?;
  need(data.points==9&&data.float4s==31&&data.records.len()==496&&data.indices.len()==1024,"830 derived9point+unit31 domain")?;
  for(b,s)in[(&data.counts,"4"),(&data.indices,"5"),(&data.records,"8")]{need(p["authored"]["slots"][s]["sha256"]==hash(b)&&p["authored"]["slots"][s]["bytes"].as_u64()==Some(b.len()as u64),"830 exact derived masks/components/index payload")?;}
  need(data.records[432..]==ground[240..]&&u32::from_le_bytes(data.records[476..480].try_into().unwrap())==u32::MAX,"830 unchanged unit tail/shadow sentinel")?;
  Ok(Self{policy:Arc::from(bytes),ground:Arc::from(ground),flying:Arc::from(flying),states:states.to_vec(),lights:[data.counts.into(),data.indices.into(),data.records.into()]})
 }
 pub fn lights(&self)->[Arc<[u8]>;3]{self.lights.clone()}
 pub fn originals(&self)->[Arc<[u8]>;3]{let states=std::array::from_fn(|i|(self.states[i].0.as_str(),self.states[i].1));let d=policy::construct(&self.ground,&self.flying,states,false).expect("sealed830 source");[d.counts.into(),d.indices.into(),d.records.into()]}
 pub fn unit(&self)->&[u8]{&self.ground[240..]}
 pub fn receipt(&self)->Value{json!({"schema":"AuthoredReceiverLight830-v1","rule":RULE,"classification":"EXPLICIT_AUTHORED_APPEARANCE_EXTENSION_NOT_ORIGINAL_PARITY","policy_bytes":self.policy.to_vec(),"ground":self.ground.to_vec(),"flying":self.flying.to_vec(),"states":self.states,"point_count":9,"float4_count":31,"record_bytes":496,"unit_index":27,"lights_sha256":self.lights.iter().map(|b|hash(b)).collect::<Vec<_>>()})}
 pub fn from_receipt(v:&Value)->Result<Self,String>{let bytes:Vec<u8>=serde_json::from_value(v["policy_bytes"].clone()).map_err(|e|e.to_string())?;let g:Vec<u8>=serde_json::from_value(v["ground"].clone()).map_err(|e|e.to_string())?;let f:Vec<u8>=serde_json::from_value(v["flying"].clone()).map_err(|e|e.to_string())?;let states:Vec<(String,u32)>=serde_json::from_value(v["states"].clone()).map_err(|e|e.to_string())?;let d=Self::seal(&bytes,&g,&f,&states)?;need(d.receipt()==*v,"830 cached authored receipt exactness")?;Ok(d)}
 pub fn terrain_lights(&self,actual:[&[u8];3])->Result<(),String>{need(actual.iter().zip(&self.lights).all(|(a,b)|*a==b.as_ref()),"830 actual current31 views differ from sealed authored source")}
}
