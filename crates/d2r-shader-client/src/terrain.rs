//! Viewer-side form of the Terrain53 API (`d2r-iridia-terrain53`). The sources are read
//! here (same paths and reader as in process) and stored as blobs; loading checks, prefix and pixel
//! preparation and the prefix validation run in the translator.

use crate::rpc::{self, Request};
use d2r_shader::{ShaderError, terrain};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn require_consumed_identity58() -> Result<(&'static str, &'static str), &'static str> {
    static ID: std::sync::OnceLock<Result<(&'static str, &'static str), &'static str>> = std::sync::OnceLock::new();
    *ID.get_or_init(|| match crate::rpc::identity() {
        Ok(id) => match &id.terrain_consumed_identity58 {
            Ok((a, b)) => Ok((leak(a), leak(b))),
            Err(e) => Err(leak(e)),
        },
        Err(e) => Err(leak(&e.to_string())),
    })
}
fn leak(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}

pub mod terrain_iridia {
    use super::*;

    pub const REVISION: &str = "terrain53-source-directed-402-iridia-provenance-v2";

    /// The four original terrain stage resources (path -> blob).
    pub struct Sources {
        pub(crate) files: BTreeMap<String, String>,
    }
    impl Sources {
        pub fn load(mut read: impl FnMut(&str) -> Result<Vec<u8>, ShaderError>) -> Result<Self, ShaderError> {
            let mut files = BTreeMap::new();
            for stage in terrain::OriginalTerrainStage::ALL {
                let p = terrain::profile(stage);
                let bytes = read(&p.path)?;
                files.insert(p.path.clone(), rpc::blob(&bytes).map_err(|m| ShaderError::Io(std::io::Error::other(m)))?);
            }
            rpc::call_shader::<()>(Request::TerrainLoad { files: files.clone() })?;
            Ok(Self { files })
        }
    }

    /// Same fields and serde form as the GPL `PreparedPrefix` (`lifts` as JSON values).
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct PreparedPrefix {
        pub revision: String,
        pub preparation_key: String,
        pub compiler_identity: Value,
        pub lifts: Vec<Value>,
        pub canonical: terrain::TerrainProgram,
        #[serde(skip)]
        pub spirv: Vec<u8>,
        pub original_source_to_402_projection: bool,
        pub native_compiler_called: bool,
        pub native_compiler_fallback_available: bool,
        pub general_tessellation_supported: bool,
        pub automatic_native_domain_producer: bool,
        pub numerical_or_raster_acceptance: bool,
    }
    /// The prefix on the wire: its serde form plus the SPIR-V the serde form skips.
    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct PrefixWire {
        pub prefix: Value,
        pub spirv: Vec<u8>,
    }

    pub fn prepare_prefix(s: &Sources) -> Result<PreparedPrefix, ShaderError> {
        let w: PrefixWire = rpc::call_shader(Request::TerrainPrefix { files: s.files.clone() })?;
        let mut p: PreparedPrefix = rpc::decode("terrain_prefix", w.prefix).map_err(|m| ShaderError::Io(std::io::Error::other(m)))?;
        p.spirv = w.spirv;
        Ok(p)
    }
    /// The translator prepares the prefix again, requires it to equal `p` and runs the GPL check.
    pub fn validate_prefix(s: &Sources, p: &PreparedPrefix) -> Result<(), ShaderError> {
        let io = |m: String| ShaderError::Io(std::io::Error::other(m));
        let wire = PrefixWire { prefix: serde_json::to_value(p).map_err(|e| io(e.to_string()))?, spirv: p.spirv.clone() };
        let prefix = rpc::blob(&serde_json::to_vec(&wire).map_err(|e| io(e.to_string()))?).map_err(io)?;
        rpc::call_shader(Request::TerrainValidatePrefix { files: s.files.clone(), prefix })
    }

    pub mod terrain_scene {
        use super::*;
        pub const REVISION: &str = "terrain53-actual-scene40-bank16off-71-result-projection-v1";

        /// The scene40 witness manifest (the translator reads and seals it at each use).
        pub struct SceneWitness {
            path: String,
        }
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct PreparedPixel {
            pub revision: String,
            pub preparation_key: String,
            pub wgsl: String,
            pub wgsl_sha256: String,
            pub bindings: Value,
            pub source_scope: Value,
            pub projection: Value,
            pub linking: Value,
            pub source_math_control_preserved: bool,
            pub physical_null_substitutes: bool,
            pub current_world_or_gpu_authority: bool,
        }
        fn path_text(path: &std::path::Path) -> Result<String, ShaderError> {
            let p = std::path::absolute(path)?;
            p.to_str().map(str::to_owned).ok_or_else(|| ShaderError::Invalid("scene40 witness path is not UTF-8".into()))
        }
        pub fn load_manifest(s: &Sources, path: &std::path::Path) -> Result<SceneWitness, ShaderError> {
            let path = path_text(path)?;
            rpc::call_shader::<()>(Request::TerrainWitness { files: s.files.clone(), witness: path.clone() })?;
            Ok(SceneWitness { path })
        }
        pub fn prepare_pixel(s: &Sources, w: &SceneWitness) -> Result<PreparedPixel, ShaderError> {
            rpc::call_shader(Request::TerrainPixel { files: s.files.clone(), witness: Some(w.path.clone()), shadow: false })
        }
        /// The in-process `validate_pixel` prepares the pixel program again from the same
        /// sources and witness and requires the same serde bytes. Here that preparation runs in the translator
        /// (witness requests are sealed at each use, never cached) and the bytes are compared on this side.
        pub fn validate_pixel(s: &Sources, w: &SceneWitness, p: &PreparedPixel) -> Result<(), ShaderError> {
            let expected = prepare_pixel(s, w)?;
            let bytes = |x: &PreparedPixel| serde_json::to_vec(x).map_err(|e| ShaderError::Unsupported(format!("Terrain53: {e}")));
            if bytes(&expected)? != bytes(p)? {
                return Err(ShaderError::Unsupported("Terrain53: source/current-input reconstructed pixel payload mismatch".into()));
            }
            Ok(())
        }
        pub fn prepare_pixel_from_sources(s: &Sources) -> Result<PreparedPixel, ShaderError> {
            rpc::call_shader(Request::TerrainPixel { files: s.files.clone(), witness: None, shadow: false })
        }
        pub mod shadow895 {
            use super::*;
            pub fn prepare_shadow_pixel(s: &Sources, w: &SceneWitness) -> Result<PreparedPixel, ShaderError> {
                rpc::call_shader(Request::TerrainPixel { files: s.files.clone(), witness: Some(w.path.clone()), shadow: true })
            }
            pub fn prepare_shadow_pixel_from_sources(s: &Sources) -> Result<PreparedPixel, ShaderError> {
                rpc::call_shader(Request::TerrainPixel { files: s.files.clone(), witness: None, shadow: true })
            }
        }
    }
}
