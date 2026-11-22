// SPDX-License-Identifier: MIT
// AUTHORED policy; original source/reference behavior remains separate.
pub const NAME:&str="SPECULAR_FROM_FLYING_PRESERVE_GROUND_DIFFUSE";
pub const IDS:[&str;4]=["fx-idle-0","fx-idle-1","fx-idle-2","fx-idle-3"];
pub struct Domain{pub counts:Vec<u8>,pub indices:Vec<u8>,pub records:Vec<u8>,pub points:usize,pub float4s:usize}
pub fn applies(id:&str,state:u32)->bool{state==0&&IDS.contains(&id)}
fn require(v:bool,why:&str)->Result<(),String>{if v{Ok(())}else{Err(why.to_owned())}}
fn source(records:&[u8])->Result<usize,String>{
 require(records.len()>=64&&(records.len()-64)%48==0,"source record/unit extent")?;
 let n=(records.len()-64)/48;require(n>=2,"source waypoints missing")?;
 for r in records[..n*48].chunks_exact(48){require(r[36..40]==0xffffffffu32.to_le_bytes(),"original packed weights must be both one")?;}
 Ok(n)
}
pub fn construct(ground:&[u8],flying:&[u8],states:[(&str,u32);4],authored:bool)->Result<Domain,String>{
 let ng=source(ground)?;let nf=source(flying)?;
 require(ng==5&&nf==6,"only admitted ground19/flying22 source domains")?;
 require(ground[(ng-2)*48..]==flying[(nf-2)*48..],"unrelated waypoints/unit differ")?;
 for(i,(id,state))in states.iter().enumerate(){require(*id==IDS[i]&&applies(id,*state),"fixed authored snapshot requires the exact four idle identities; no flight/collected promotion")?;}
 let mut records=Vec::new();
 if authored{
  for(row,mask)in ground[..(ng-2)*48].chunks_exact(48).map(|r|(r,0x0000ffffu32)).chain(flying[..(nf-2)*48].chunks_exact(48).map(|r|(r,0xffff0000u32))){
   let start=records.len();records.extend_from_slice(row);records[start+36..start+40].copy_from_slice(&mask.to_le_bytes());
  }
  records.extend_from_slice(&ground[(ng-2)*48..]);
 }else{records.extend_from_slice(ground);}
 let points=(records.len()-64)/48;let float4s=records.len()/16;
 require(points<64&&(records.len()-64)%48==0&&float4s==points*3+4,"bounded derived typed domain")?;
 let counts=(((points as u32)<<6)|(1u32<<24)).to_le_bytes().to_vec();
 let mut indices=vec![0u8;1024];for i in 0..=points{indices[i*4..i*4+4].copy_from_slice(&((i*3)as u32).to_le_bytes());}
 Ok(Domain{counts,indices,records,points,float4s})
}
