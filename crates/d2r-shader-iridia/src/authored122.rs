//! Authored-only transport fixture. Not a PreparedShader or an admission capability.
use crate::{payload104::ShaderPayload104,precision_carrier::internal_spirv_options};
use iridia_naga as ir;
pub fn transport(text:&str,old:bool)->ShaderPayload104 {
 let m=ir::front::wgsl::parse_str(text).unwrap();
 let info=ir::valid::Validator::new(ir::valid::ValidationFlags::all(),ir::valid::Capabilities::all()).validate(&m).unwrap();
 let options=if old {crate::precision_carrier::stock_debug_spirv_options()}else{internal_spirv_options()};
 let (words,_)=ir::back::spv::write_vec_with_report(&m,&info,&options,Some(&ir::back::spv::PipelineOptions{shader_stage:m.entry_points[0].stage,entry_point:"main".into()})).unwrap();
 let p=ShaderPayload104::AuthoredSpirv{words:words.into()};p.validate().unwrap();p
}
