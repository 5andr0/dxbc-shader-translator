//! Embed a pre-build authenticated Cargo closure. Never invoke Cargo here.
use sha2::{Digest,Sha256};
use serde_json::Value;
use std::{collections::{BTreeMap,BTreeSet},fs,path::{Path,PathBuf}};
fn sha(b:&[u8])->String{format!("{:x}",Sha256::digest(b))}
fn canonical(v:Value)->Value{match v{Value::Object(m)=>Value::Object(m.into_iter().map(|(k,v)|(k,canonical(v))).collect::<BTreeMap<_,_>>().into_iter().collect()),Value::Array(a)=>Value::Array(a.into_iter().map(canonical).collect()),v=>v}}
fn text<'a>(v:&'a Value,k:&str)->&'a str{v[k].as_str().unwrap_or_else(||panic!("build identity requires string {k}"))}
fn digest(v:&str)->bool{v.len()==64&&v.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b))}
fn safe_path(p:&str)->bool{!p.is_empty()&&!p.contains('\\')&&!p.contains(':')&&!p.starts_with('/')&&p.split('/').all(|s|!s.is_empty()&&s!="."&&s!="..")}
fn authored(root:&Path,prefix:&Path,out:&mut BTreeSet<String>){for e in fs::read_dir(root.join(prefix)).expect("authored source directory"){let e=e.expect("authored source entry");let path=prefix.join(e.file_name());if e.file_type().expect("authored source type").is_dir(){authored(root,&path,out)}else {out.insert(path.to_string_lossy().replace('\\',"/"));}}}
fn verified_json(file:&str,expected:&str,bound:usize)->(Vec<u8>,Value){assert!(digest(expected),"expected canonical file SHA");println!("cargo:rerun-if-changed={file}");let b=fs::read(file).expect("canonical identity/map file");assert!(b.len()<=bound,"canonical file extent");assert_eq!(sha(&b),expected,"canonical file checksum");let v:Value=serde_json::from_slice(&b).expect("canonical JSON");assert_eq!(serde_json::to_vec(&canonical(v.clone())).unwrap(),b,"canonical JSON without whitespace required");(b,v)}
fn include_macros(tokens:proc_macro2::TokenStream,out:&mut Vec<(String,proc_macro2::TokenStream)>){
 let t=tokens.into_iter().collect::<Vec<_>>();for i in 0..t.len(){
  if let (Some(proc_macro2::TokenTree::Ident(name)),Some(proc_macro2::TokenTree::Punct(bang)),Some(proc_macro2::TokenTree::Group(args)))=(t.get(i),t.get(i+1),t.get(i+2)){
   let name=name.to_string();if bang.as_char()=='!'&&matches!(name.as_str(),"include"|"include_str"|"include_bytes"|"include_wgsl"|"include_spirv"|"include_spirv_raw"|"include_spirv_source"){out.push((name,args.stream()));}
  }
  if let proc_macro2::TokenTree::Group(g)=&t[i]{include_macros(g.stream(),out)}
 }
}
fn verify_path_package(package:&Value,root:&Path)->BTreeMap<String,String>{
 let root=root.canonicalize().expect("actual path package directory");let mut files=BTreeMap::new();let mut previous="";
 for row in package["authored_sources"].as_array().expect("every path package requires authored_sources"){
  let path=text(row,"path");let hash=text(row,"sha256");assert!(safe_path(path)&&path>previous&&digest(hash),"path package source path/order/hash");previous=path;
  let actual=root.join(path).canonicalize().expect("actual path package source");assert!(actual.starts_with(&root),"path package source escapes package");assert_eq!(sha(&fs::read(&actual).expect("path source bytes")),hash,"path package authored source changed after driver");println!("cargo:rerun-if-changed={}",actual.display());files.insert(path.to_string(),hash.to_string());
 }
 let mut required=BTreeSet::from(["Cargo.toml".to_string()]);if root.join("build.rs").is_file(){required.insert("build.rs".into());}if root.join("src").is_dir(){authored(&root,Path::new("src"),&mut required);}assert!(required.iter().all(|p|files.contains_key(p)),"path package source coverage incomplete");
 let generated=package["generated_sources"].as_array().expect("explicit generated source inventory");let mut generated_seen=BTreeSet::new();
 let mut queue=files.keys().filter(|p|p.ends_with(".rs")).cloned().collect::<BTreeSet<_>>();let mut parsed=BTreeSet::new();
 while let Some(source)=queue.pop_first(){if !parsed.insert(source.clone()){continue}let text=fs::read_to_string(root.join(&source)).expect("Rust source UTF8");let tokens=text.parse::<proc_macro2::TokenStream>().expect("Rust source tokenization");let mut includes=vec![];include_macros(tokens,&mut includes);
  for (name,args) in includes {
   if let Ok(literal)=syn::parse2::<syn::LitStr>(args.clone()){
    let path=root.join(Path::new(&source).parent().unwrap()).join(literal.value()).canonicalize().expect("literal include source");assert!(path.starts_with(&root),"literal include outside package requires explicit separate support");let relative=path.strip_prefix(&root).unwrap().to_string_lossy().replace('\\',"/");assert!(files.contains_key(&relative),"literal include missing from authored_sources: {source} -> {relative}");if name=="include"{queue.insert(relative);}
   }else{
    let normalized=args.to_string().chars().filter(|c|!c.is_whitespace()).collect::<String>();
    // Exact official forwarding definitions only; literal call sites are inventoried.
    if package["name"]=="wgpu" && source=="src/macros/mod.rs" && normalized=="$($token)*" {
     assert_eq!(sha(text.as_bytes()),"d5b9c469c67c2976a1c92e9d160888744c41880d6b28262dabc507cd337d53c5","wgpu forwarding macro definition changed");continue;
    }
    let path=if normalized=="concat!(env!(\"OUT_DIR\"),\"/consumed_build.rs\")"&&source=="src/build_identity.rs"{"OUT_DIR/consumed_build.rs"}else if normalized=="concat!(env!(\"OUT_DIR\"),\"/terrain_consumed_build58.rs\")"&&source=="src/lib.rs"{"OUT_DIR/terrain_consumed_build58.rs"}else{panic!("unresolved include macro: {source} {name}")};
    assert_eq!(name,"include","only owned generated Rust hooks are admitted");assert!(files.contains_key("build.rs"),"generated hook has no authored producer");let matches=generated.iter().filter(|r|r["source"]==source&&r["path"]==path&&r["producer"]=="build.rs"&&r["recipe"]=="verified-consumed-identity-v2").count();assert_eq!(matches,1,"exact generated hook recipe required");generated_seen.insert((source.clone(),path.to_string()));
   }
  }
 }
 assert_eq!(generated_seen.len(),generated.len(),"unobserved/duplicate generated source recipe");files
}
fn main(){
 for name in ["D2R_IRIDIA_BUILD_IDENTITY_FILE","D2R_IRIDIA_BUILD_IDENTITY_SHA256","D2R_IRIDIA_BUILD_SOURCE_MAP_FILE","D2R_IRIDIA_BUILD_SOURCE_MAP_SHA256"]{println!("cargo:rerun-if-env-changed={name}");}
 let file=std::env::var("D2R_IRIDIA_BUILD_IDENTITY_FILE").ok();let expected=std::env::var("D2R_IRIDIA_BUILD_IDENTITY_SHA256").ok();
 let (qualified,json,digest_value)=match (file,expected){
  (None,None)=>{assert!(std::env::var_os("CARGO_FEATURE_REQUIRE_CONSUMED_BUILD").is_none(),"production admission requires actual consumed build identity from the prebuild driver");assert!(std::env::var_os("D2R_IRIDIA_BUILD_SOURCE_MAP_FILE").is_none()&&std::env::var_os("D2R_IRIDIA_BUILD_SOURCE_MAP_SHA256").is_none(),"source map without identity");(false,"null".to_string(),sha(include_bytes!("Cargo.lock")))},
  (Some(file),Some(expected))=>{
   let (bytes,v)=verified_json(&file,&expected,4*1024*1024);
   assert_eq!(text(&v,"schema"),"d2r-iridia-consumed-build-v2");assert_eq!(text(&v,"target"),std::env::var("TARGET").unwrap(),"build identity target differs from Cargo target");
   let map_file=std::env::var("D2R_IRIDIA_BUILD_SOURCE_MAP_FILE").expect("path package source map required");let map_sha=std::env::var("D2R_IRIDIA_BUILD_SOURCE_MAP_SHA256").expect("source map checksum required");let (_,map)=verified_json(&map_file,&map_sha,1024*1024);assert_eq!(text(&map,"schema"),"d2r-iridia-source-map-v1");let mut directories=BTreeMap::new();for row in map["packages"].as_array().expect("source map packages"){assert!(directories.insert(text(row,"id").to_string(),PathBuf::from(text(row,"directory"))).is_none(),"duplicate source map id");}
   let request=&v["requested_build"];assert!(!text(request,"package").is_empty());assert!(request["no_default_features"].is_boolean());let features=request["features"].as_array().expect("requested features");let mut prev="";for f in features{let s=f.as_str().expect("feature string");assert!(s>prev,"requested features sorted/unique");prev=s;}
   let packages=v["packages"].as_array().expect("normal/build package closure");assert!(!packages.is_empty()&&packages.len()<=4096,"bounded package closure");
   let mut ids=BTreeSet::new();let mut pinned=BTreeSet::new();let mut last=String::new();
   let own_root=PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).canonicalize().expect("own package directory");let mut own_files=None;
   for p in packages{
    let id=text(p,"id");assert!(id>last.as_str(),"package closure sorted/unique");last=id.into();assert!(!id.contains('\\')&&!id.contains("file://")&&!id.contains("path+"),"machine path in package identity");ids.insert(id.to_string());
    let name=text(p,"name");let version=text(p,"version");let source=&p["source"];let checksum=&p["checksum"];if !source.is_null(){assert!(p.get("authored_sources").is_none()&&p.get("generated_sources").is_none(),"registry source cannot impersonate authored package");let s=source.as_str().expect("registry source");assert!((s.starts_with("registry+")||s.starts_with("git+"))&&!s.contains("file://"),"package source");if s.starts_with("registry+"){assert!(digest(checksum.as_str().expect("registry checksum")),"package checksum");}}else{assert!(checksum.is_null(),"authored package has registry checksum");let directory=directories.get(id).expect("path package absent from source map");let files=verify_path_package(p,directory);if name==std::env::var("CARGO_PKG_NAME").unwrap(){assert_eq!(version,std::env::var("CARGO_PKG_VERSION").unwrap());assert_eq!(directory.canonicalize().unwrap(),own_root,"own source map directory differs from actual Cargo package");assert!(own_files.replace(files).is_none(),"duplicate own package");}}
    if matches!(name,"iridia"|"iridia-dxbc"|"iridia-ir"|"iridia-build"|"iridia-emit"|"primase-naga"|"naga"){if name=="primase-naga"||name=="naga"{assert_eq!(version,"30.0.1")}else{assert_eq!(version,"0.5.0")}pinned.insert(name.to_string());}
    for k in ["features","dependencies"] {let a=p[k].as_array().unwrap_or_else(||panic!("package {k}"));let mut previous="";for s in a{let s=s.as_str().expect("closure string");assert!(s>previous,"package list sorted/unique");previous=s;}}
   }
   for n in ["iridia","iridia-dxbc","primase-naga","naga"]{assert!(pinned.contains(n),"pinned consumed package absent: {n}");}
   for p in packages{for d in p["dependencies"].as_array().unwrap(){assert!(ids.contains(d.as_str().unwrap()),"dependency edge outside recorded closure");}}
   let root=own_root;let required=own_files.expect("own authored path package absent");
   let passes=v["authored_passes"].as_array().expect("authored pass identities");let mut seen=BTreeSet::new();let mut previous="";
   for p in passes{let path=text(p,"path");assert!(safe_path(path)&&path>previous,"authored paths safe/sorted/unique");previous=path;assert_eq!(required.get(path).map(String::as_str),Some(text(p,"sha256")),"own pass differs from path package source");assert_eq!(sha(&fs::read(root.join(path)).expect("authored source")),text(p,"sha256"),"authored source changed after driver");println!("cargo:rerun-if-changed={path}");seen.insert(path.to_string());}
   assert_eq!(seen,required.keys().cloned().collect(),"complete authored adapter source required");
   (true,String::from_utf8(bytes).unwrap(),expected)
  },
  _=>panic!("build identity file and expected SHA must both be provided"),
 };
 let out=PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("consumed_build.rs");
 fs::write(out,format!("pub const CONSUMED_BUILD_QUALIFIED:bool={qualified};\npub const BUILD_IDENTITY_JSON:&str={json:?};\npub const BUILD_IDENTITY_SHA256:&str={digest_value:?};\n")).expect("build identity embed");
}
