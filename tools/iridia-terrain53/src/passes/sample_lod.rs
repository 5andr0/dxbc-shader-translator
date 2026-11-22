//! Source-preserving repair for the Iridia0.5.0 mixed depth/ordinary sample LOD.
//!
//! DXBC `sample_l ... l(0)` has an explicit zero mip. Iridia marks a texture
//! depth when comparison instructions also use it, but leaves ordinary sample_l
//! LOD as f32. WGSL/Naga depth Exact requires integer, unlike ordinary sampled
//! textures. Use an existing earlier integer-zero Exact operand ONLY for direct positive f32
//! zero. No cast, resource class, sampler, coordinates or fractional LOD changes.
use iridia_naga::{Expression as E, Function, Handle, ImageClass, Literal, Module, SampleLevel, ScalarKind, TypeInner};

pub const SAMPLE_LOD_REVISION: &str = "depth-exact-positive-zero-integer-handle-v2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleLodSite {
    pub function: String,
    pub expression: usize,
    pub image: usize,
    pub old_lod_expression: usize,
    pub new_lod_expression: usize,
    pub original_f32_bits: u32,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SampleLodReport {
    pub repaired: Vec<SampleLodSite>,
    pub integer_depth_exact_preserved: usize,
    pub ordinary_exact_preserved: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SampleLodError {
    TypeResolution { function: String, reason: String },
    UnsupportedDepthExact { function: String, expression: usize, reason: String },
}
impl std::fmt::Display for SampleLodError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {write!(f,"{self:?}")}
}
impl std::error::Error for SampleLodError {}

#[derive(Clone, Copy)]
enum FnKey { Regular(Handle<Function>), Entry(usize) }

fn scan(module:&Module,function:&Function,key:FnKey,name:String,report:&mut SampleLodReport,edits:&mut Vec<(FnKey,Handle<E>,Handle<E>)>)->Result<(),SampleLodError> {
    let mut types=iridia_naga::front::Typifier::new();
    let ctx=iridia_naga::proc::ResolveContext::with_locals(module,&function.local_variables,&function.arguments);
    for (h,e) in function.expressions.iter() {
        let E::ImageSample{image,level:SampleLevel::Exact(lod),depth_ref,..}=*e else {continue};
        types.grow(h,&function.expressions,&ctx).map_err(|e|SampleLodError::TypeResolution{function:name.clone(),reason:e.to_string()})?;
        let TypeInner::Image{class:ImageClass::Depth{multi:false},..}=types.get(image,&module.types) else {report.ordinary_exact_preserved+=1;continue};
        match types.get(lod,&module.types) {
            TypeInner::Scalar(s) if matches!(s.kind,ScalarKind::Sint|ScalarKind::Uint) => {report.integer_depth_exact_preserved+=1;}
            TypeInner::Scalar(s) if s.kind==ScalarKind::Float && s.width==4 => {
                if depth_ref.is_some() {return Err(SampleLodError::UnsupportedDepthExact{function:name,expression:h.index(),reason:"unexpected comparison Exact float LOD; source instruction mapping required".into()});}
                let E::Literal(Literal::F32(value))=function.expressions[lod] else {return Err(SampleLodError::UnsupportedDepthExact{function:name,expression:h.index(),reason:"dynamic/indirect float LOD is not a proven zero mip".into()});};
                if value.to_bits()!=0 {return Err(SampleLodError::UnsupportedDepthExact{function:name,expression:h.index(),reason:format!("float LOD bits {:08x} are not direct positive zero; no truncation or approximation",value.to_bits())});}
                let integer_zero=function.expressions.iter().take(h.index()).find_map(|(z,e)|match e{E::Literal(Literal::U32(0)|Literal::I32(0))=>Some(z),_=>None}).ok_or_else(||SampleLodError::UnsupportedDepthExact{function:name.clone(),expression:h.index(),reason:"no earlier integer-zero expression; arena insertion not authorized by this pass".into()})?;
                report.repaired.push(SampleLodSite{function:name.clone(),expression:h.index(),image:image.index(),old_lod_expression:lod.index(),new_lod_expression:integer_zero.index(),original_f32_bits:0});
                edits.push((key,h,integer_zero));
            }
            other => return Err(SampleLodError::UnsupportedDepthExact{function:name,expression:h.index(),reason:format!("unsupported depth LOD type {other:?}")}),
        }
    }
    Ok(())
}

fn repair_module(module:&mut Module)->Result<SampleLodReport,SampleLodError> {
    let mut report=SampleLodReport::default();let mut edits=vec![];
    for (h,f) in module.functions.iter(){scan(module,f,FnKey::Regular(h),format!("function:{}:{}",h.index(),f.name.as_deref().unwrap_or("")),&mut report,&mut edits)?;}
    for (index,ep) in module.entry_points.iter().enumerate(){scan(module,&ep.function,FnKey::Entry(index),format!("entry:{index}:{}",ep.name),&mut report,&mut edits)?;}
    // No mutation is made until every source sample is classified successfully.
    for (key,h,integer_zero) in edits {
        let f=match key{FnKey::Regular(h)=>module.functions.get_mut(h),FnKey::Entry(index)=>&mut module.entry_points[index].function};
        if let E::ImageSample{level,..}=&mut f.expressions[h]{*level=SampleLevel::Exact(integer_zero);}else{unreachable!("immutable scan recorded an ImageSample")}
    }
    Ok(report)
}

pub fn repair_exact_lod(program:&mut iridia::Program)->Result<SampleLodReport,SampleLodError> {repair_module(&mut program.module)}

