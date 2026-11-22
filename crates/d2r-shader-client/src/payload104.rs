//! Copy of the project's own d2r-shader-iridia payload104.rs. `controlled` returns the
//! translator's payload; the qualification plan text comes from the translator identity.
//! Immutable CPU payload. Serialized evidence is consistency data, never device or source admission.
use crate::{Error,Result,sha256};
use serde::{Serialize,Deserialize};
use std::sync::Arc;
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="kind",deny_unknown_fields)]
pub enum ShaderPayload104 {
    Wgsl { text:Arc<str> },
    /// Only the qualification factory may consume this; no original source receipt.
    AuthoredSpirv { words:Arc<[u32]> },
    /// Internal Naga coordinates; final backend conversion is still required.
    ControlledSpirv { words:Arc<[u32]>, evidence:PrecisionEvidence104 },
}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrecisionEvidence104 {
    source_sha256:Option<String>, receipt:String, correspondence:String, qualification:String,
    primase_package:String, stock_package:String, final_module:String,
    /// R2 (carrier rule 4) integer NoContraction targets the writer dropped from these words
    /// (result ids, this writer execution's record). Every re-import restores exactly these controls, so
    /// the final module keeps every source control. Absent (empty) for modules without integer drops.
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    r2_dropped_integer_no_contraction:Vec<u32>,
}
impl ShaderPayload104 {
    /// The translator built this payload from the precise artifact (GPL `controlled`).
    pub fn controlled(c:&crate::precision_carrier::PreparedSpirv)->Result<Self>{
        c.wire().controlled.clone()
    }
    pub fn source_sha256(&self)->Option<&str>{match self{Self::ControlledSpirv{evidence,..}=>evidence.source_sha256.as_deref(),_=>None}}
    pub fn kind(&self)->&'static str {match self {Self::Wgsl{..}=>"wgsl",Self::AuthoredSpirv{..}=>"authored-spirv104",Self::ControlledSpirv{..}=>"controlled-spirv104"}}
    /// Includes precision evidence as well as bytes; CacheIdentity binds this digest.
    pub fn bytes_sha256(&self)->String{match self{Self::Wgsl{text}=>sha256(text.as_bytes()),Self::AuthoredSpirv{words}|Self::ControlledSpirv{words,..}=>sha256(&words.iter().flat_map(|w|w.to_le_bytes()).collect::<Vec<_>>())}}
    pub fn digest(&self)->String {crate::canonical_metadata_sha256(self).expect("payload serializes")}
    pub fn wgsl(&self)->Result<&str>{match self {Self::Wgsl{text}=>Ok(text),_=>Err(Error::Unsupported("controlled payload has no WGSL representation".into()))}}
    pub fn module(&self)->Result<naga::Module>{
        match self {
            Self::Wgsl{text}=>naga::front::wgsl::parse_str(text).map_err(|e|Error::Validation(e.emit_to_string(text))),
            // An R2 module cannot carry integer NoContraction in its words; the payload's record restores
            // exactly those controls through the same checked restore as `prepare_spirv`.
            Self::ControlledSpirv{words,evidence} if !evidence.r2_dropped_integer_no_contraction.is_empty()=>{
                let (mut m,report)=naga::front::spv::Frontend::new(words.iter().copied(),&Self::import_options()).parse_with_report().map_err(|e|Error::Validation(e.to_string()))?;
                crate::precision_carrier::restore_r2_dropped_integers(&mut m,&report,&evidence.r2_dropped_integer_no_contraction).map_err(Error::Validation)?;
                Ok(m)
            }
            Self::ControlledSpirv{words,..}|Self::AuthoredSpirv{words}=>naga::front::spv::Frontend::new(words.iter().copied(),&Self::import_options()).parse().map_err(|e|Error::Validation(e.to_string())),
        }
    }
    fn import_options()->naga::front::spv::Options{naga::front::spv::Options{adjust_coordinate_space:false,strict_capabilities:true,block_ctx_dump_prefix:None}}
    pub fn validate(&self)->Result<()> {
        let m=self.module()?;
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(),naga::valid::Capabilities::all()).validate(&m).map_err(|e|Error::Validation(format!("final payload module: {e:?}")))?;
        if let Self::ControlledSpirv{evidence,..}=self {
            if evidence.stock_package!="95ccfe0995d3a31d39edc6208039bd44d8bbe14a998fa2340ea1a5036bc8ee82" || evidence.primase_package!="8cbf6658602ecb8f480292ac77f6fc9155b9f9462d23019318be73777dbdbe10" || evidence.final_module!=sha256(format!("{m:?}").as_bytes()) || evidence.qualification!=crate::rpc::identity()?.qualification_plan || evidence.receipt.is_empty() || evidence.correspondence.is_empty() {
                return Err(Error::Identity("controlled payload package/module/qualification".into()))
            }
        }
        Ok(())
    }
}
