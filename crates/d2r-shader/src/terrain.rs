//! Exact original Terrain four-stage identity and fixed-factor composition.
//! A compiled function does not establish fixed-function tessellator domain/cycle bits.
use crate::{
    ShaderError, sha256,
    terrain_tokens::{self as tokens, Index},
};
use serde::{Deserialize, Serialize};

pub const PROFILE: &str = "original-terrain0-fractional-odd-factor1-explicit-domain-v1";
pub const REVISION: &str = "terrain-original-token-typed-402-node-vertex-v1";
pub const PIXEL_PROFILE: &str = "original-terrain0-pixel-source-v1";
pub const REMAP_REVISION: &str = "vkd3d2.1-terrain-set1-cbv0-srv32-sampler128-uav160-v1";
pub const WIDTHS: [u8; 6] = [4, 4, 3, 3, 3, 4];
pub use crate::terrain_compose::{GeometryAbi, TerrainProgram, compose};
pub use crate::terrain_tokens::{Node, Op, Program, Signature, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OriginalTerrainStage {
    Vertex,
    Hull,
    Domain,
    Fragment,
}
impl OriginalTerrainStage {
    pub fn name(self) -> &'static str {
        match self {
            Self::Vertex => "vs",
            Self::Hull => "hs",
            Self::Domain => "ds",
            Self::Fragment => "ps",
        }
    }
    pub(crate) fn token_type(self) -> u32 {
        match self {
            Self::Vertex => 1,
            Self::Hull => 3,
            Self::Domain => 4,
            Self::Fragment => 0,
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Vertex => 0,
            Self::Hull => 1,
            Self::Domain => 2,
            Self::Fragment => 3,
        }
    }
    pub const ALL: [Self; 4] = [Self::Vertex, Self::Hull, Self::Domain, Self::Fragment];
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageProfile {
    pub stage: OriginalTerrainStage,
    pub path: String,
    pub resource_sha256: String,
    pub resource_bytes: usize,
    pub offset: usize,
    pub byte_count: usize,
    pub dxbc_sha256: String,
}
pub fn profile(stage: OriginalTerrainStage) -> StageProfile {
    let (path, resource_sha256, resource_bytes, byte_count, dxbc_sha256) = match stage {
        OriginalTerrainStage::Vertex => (
            "vs_terrain.bin",
            "0eb238deba46e7694979690a61f1d2fdf44d32d98862c81d9b113c291e8f0f2e",
            2100,
            1864,
            "2c12f546abbf316623b09f3d102e5f341cf910ec0e74abd2852b6473f5fd8cfc",
        ),
        OriginalTerrainStage::Hull => (
            "hs_terrain_h.bin",
            "405c6e19d820278a0d4dbd0554bf8db26cf6edc1d28a987de8e46c093b15da67",
            1088,
            852,
            "eb5e004bdad33b588e7af8676bd92b42a258c8ae60617fa3672b6c1424a95197",
        ),
        OriginalTerrainStage::Domain => (
            "ds_terrain_d.bin",
            "2a7b77436696da20366d9ca0106bdc60024d4d90c73e2fd817c3761fa404c1d0",
            1684,
            1448,
            "f2d609f0b8fb85018cdb8426a90ce9070d142db79b44c1a82937976eebc2f8c0",
        ),
        OriginalTerrainStage::Fragment => (
            "ps_terrain.bin",
            "984443d939ac2d1246fe621793490981b20f38fda4bf05de0b3eb7f53ea921fa",
            52596,
            52360,
            "6bf62d4eb835f430734fef9ef1474e4a25ea667ca8d149932e8ee8cb5dbbee37",
        ),
    };
    StageProfile {
        stage,
        path: format!("data/hd/shaders/{path}"),
        resource_sha256: resource_sha256.into(),
        resource_bytes,
        offset: 236,
        byte_count,
        dxbc_sha256: dxbc_sha256.into(),
    }
}
pub fn fragment_profile() -> crate::compiler::OriginalProfile {
    let p = profile(OriginalTerrainStage::Fragment);
    crate::compiler::OriginalProfile {
        identity: PIXEL_PROFILE.into(),
        stage: crate::compiler::Stage::Fragment,
        path: p.path,
        resource_sha256: p.resource_sha256,
        resource_bytes: p.resource_bytes,
        offset: p.offset,
        byte_count: p.byte_count,
        dxbc_sha256: p.dxbc_sha256,
        descriptor_count: 30,
    }
}
pub fn is_fragment(p: &crate::compiler::OriginalProfile) -> bool {
    *p == fragment_profile()
}
pub fn translate_fragment(
    s: &TerrainSources,
    c: &crate::compiler::CompilerModule,
) -> Result<(Vec<u8>, Vec<crate::compiler::DescriptorBinding>, String), ShaderError> {
    c.translate(&fragment_profile(), s.code(OriginalTerrainStage::Fragment))
}
fn bad(s: &str) -> ShaderError {
    ShaderError::Unsupported(format!("original Terrain: {s}"))
}
/// The original program container of one Terrain stage inside its installed shader
/// resource: the pinned slice when the resource is the pinned one; otherwise, under
/// `D2R_VERSION_TOLERANT=1`, [`crate::compiler::tolerant_dxbc`] (pinned program found
/// anywhere in the resource, else the installed program at the pinned offset; both warn).
pub fn original_code<'a>(p: &StageProfile, resource: &'a [u8]) -> Result<&'a [u8], ShaderError> {
    let exact = (resource.len() == p.resource_bytes && sha256(resource) == p.resource_sha256)
        .then(|| p.offset.checked_add(p.byte_count).and_then(|end| resource.get(p.offset..end)))
        .flatten()
        .filter(|b| sha256(b) == p.dxbc_sha256);
    if let Some(b) = exact {
        return Ok(b);
    }
    crate::compiler::tolerant_dxbc(&p.path, resource, p.offset, p.byte_count, &p.dxbc_sha256)
        .ok_or_else(|| bad("original resource differs from the pinned one (set D2R_VERSION_TOLERANT=1 to search the installed resource)"))
}
/// Owns authenticated original bytes. No summary/default/Deserialize constructor.
pub struct TerrainSources {
    code: [Vec<u8>; 4],
}
impl TerrainSources {
    pub fn load(
        mut read: impl FnMut(&str) -> Result<Vec<u8>, ShaderError>,
    ) -> Result<Self, ShaderError> {
        let mut code: [Vec<u8>; 4] = std::array::from_fn(|_| vec![]);
        for stage in OriginalTerrainStage::ALL {
            let p = profile(stage);
            let resource = read(&p.path)?;
            let b = original_code(&p, &resource)?;
            tokens::chunks(b)?;
            code[stage.index()] = b.to_vec();
        }
        Ok(Self { code })
    }
    pub(crate) fn code(&self, stage: OriginalTerrainStage) -> &[u8] {
        &self.code[stage.index()]
    }
    /// The original program container of `stage` (as selected by [`original_code`]).
    pub fn original(&self, stage: OriginalTerrainStage) -> &[u8] {
        &self.code[stage.index()]
    }
    pub fn identities(&self) -> Vec<StageProfile> {
        OriginalTerrainStage::ALL.into_iter().map(profile).collect()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HullContract {
    pub input_control_points: u32,
    pub output_control_points: u32,
    pub domain: String,
    pub partitioning: String,
    pub output_primitive: String,
    pub factor_bits: [u32; 4],
    pub no_control_point_phase: bool,
    pub original_instructions: usize,
}
fn indices(o: &tokens::Operand, wanted: &[u32]) -> bool {
    o.indices.len() == wanted.len()
        && o.indices
            .iter()
            .zip(wanted)
            .all(|(a, b)| matches!(a,Index::Immediate(n)if n==b))
}
pub(crate) fn hull(b: &[u8]) -> Result<HullContract, ShaderError> {
    let (ins, _, _) = tokens::instructions(OriginalTerrainStage::Hull, b)?;
    // Expected operation sequence checked against the installed shader.
    let expected = [
        113, 147, 148, 149, 150, 151, 106, 115, 153, 95, 103, 103, 103, 104, 91, 54, 54, 62, 115,
        103, 54, 62,
    ];
    if ins.iter().map(|i| i.op).ne(expected) {
        return Err(bad("unknown hull phase/operation"));
    }
    for (n, value) in [(1, 3), (2, 3), (3, 2), (4, 3), (5, 3), (6, 1)] {
        if ins[n].words.len() != 1 || (ins[n].words[0] >> 11) & 255 != value {
            return Err(bad("hull count/domain/partition/winding/global flags"));
        }
    }
    if ins[8].words.get(1) != Some(&3) || ins[8].words.len() != 2 {
        return Err(bad("hull fork invocation count"));
    }
    let operands = |n: usize| -> Result<(tokens::Operand, tokens::Operand), ShaderError> {
        let mut at = 1;
        let d = tokens::operand(&ins[n].words, &mut at, 0)?;
        let s = tokens::operand(&ins[n].words, &mut at, 0)?;
        if at != ins[n].words.len() {
            return Err(bad("hull operand arity"));
        }
        Ok((d, s))
    };
    let (d, s) = operands(15)?;
    if d.reg != 0 || !indices(&d, &[0]) || d.lanes != [0] || s.reg != 23 || !s.indices.is_empty() {
        return Err(bad("raw UINT fork-ID transport"));
    }
    for n in [16, 20] {
        let (d, s) = operands(n)?;
        if d.reg != 2
            || d.lanes != [0]
            || s.reg != 4
            || s.literals != [0x3f800000]
            || s.dimension != 1
        {
            return Err(bad("literal-one hull factor producer"));
        }
        if n == 20 {
            if !indices(&d, &[3]) {
                return Err(bad("inside factor target"));
            }
        } else {
            match d.indices.as_slice() {
                [Index::Relative { offset: 0, value }]
                    if value.reg == 0 && indices(value, &[0]) && value.lanes == [0, 0, 0, 0] => {}
                _ => return Err(bad("fork target index")),
            }
        }
    }
    Ok(HullContract {
        input_control_points: 3,
        output_control_points: 3,
        domain: "tri".into(),
        partitioning: "fractional_odd".into(),
        output_primitive: "triangle_cw".into(),
        factor_bits: [0x3f800000; 4],
        no_control_point_phase: true,
        original_instructions: ins.len(),
    })
}
fn same_signature(a: &[tokens::Signature], b: &[tokens::Signature]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            a.semantic == b.semantic
                && a.semantic_index == b.semantic_index
                && a.system_value == b.system_value
                && a.component_type == b.component_type
                && a.register == b.register
                && a.mask == b.mask
        })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedTerrain {
    pub stages: Vec<StageProfile>,
    pub hull: HullContract,
    pub vertex: tokens::Program,
    pub domain: tokens::Program,
    pub fragment_inputs: Vec<tokens::Signature>,
    pub refactoring_allowed: bool,
    pub native_domain_bits_or_cycle_proved: bool,
}
pub fn inspect(s: &TerrainSources) -> Result<TypedTerrain, ShaderError> {
    let vertex = tokens::pure(
        OriginalTerrainStage::Vertex,
        s.code(OriginalTerrainStage::Vertex),
    )?;
    let domain = tokens::pure(
        OriginalTerrainStage::Domain,
        s.code(OriginalTerrainStage::Domain),
    )?;
    let h = hull(s.code(OriginalTerrainStage::Hull))?;
    let (_, hi, ho) = tokens::instructions(
        OriginalTerrainStage::Hull,
        s.code(OriginalTerrainStage::Hull),
    )?;
    let (_, fi, _) = tokens::instructions(
        OriginalTerrainStage::Fragment,
        s.code(OriginalTerrainStage::Fragment),
    )?;
    if !same_signature(&vertex.output_signature, &hi)
        || !same_signature(&hi, &ho)
        || !same_signature(&ho, &domain.inputs)
        || !same_signature(&domain.inputs, &domain.output_signature)
        || !same_signature(&domain.output_signature, &fi)
    {
        return Err(bad("complete original VS/HS/DS/PS interface chain"));
    }
    if vertex.nodes.len() != 113
        || vertex.source_value_instructions != 37
        || domain.nodes.len() != 63
        || domain.source_value_instructions != 18
        || vertex.consumed_cb_lanes.len() != 59
        || vertex
            .cb_vectors
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>()
            != [(0, 4), (3, 33)]
        || !domain.cb_vectors.is_empty()
    {
        return Err(bad("complete original value/declaration closure"));
    }
    for p in [&vertex, &domain] {
        if p.output_signature.len() != 6
            || p.output_signature
                .iter()
                .zip(WIDTHS)
                .enumerate()
                .any(|(n, (v, w))| {
                    v.register != n as u32 || v.mask != (1 << w) - 1 || v.component_type != 3
                })
        {
            return Err(bad("six original f32 output shapes"));
        }
    }
    Ok(TypedTerrain {
        stages: s.identities(),
        hull: h,
        vertex,
        domain,
        fragment_inputs: fi,
        refactoring_allowed: true,
        native_domain_bits_or_cycle_proved: false,
    })
}

/// Static original-program key, intentionally independent of frame/domain data.
/// Domain coordinate production and the actual bound uploads are not attested.
pub fn composed_vertex_key(s: &TerrainSources) -> Result<String, ShaderError> {
    let key = serde_json::json!({"schema":1,"profile":PROFILE,"revision":REVISION,"stages":s.identities(),"target":"composed-vertex-wgsl-naga30","naga":crate::NAGA_VERSION,"geometry":GeometryAbi::exact()});
    Ok(sha256(
        &serde_json::to_vec(&key).map_err(|e| ShaderError::Invalid(e.to_string()))?,
    ))
}
/// Rederive from actual authenticated original bytes, not cache-summary claims.
pub fn validate_composed_vertex(
    s: &TerrainSources,
    a: &TerrainProgram,
    bytes: &[u8],
) -> Result<(), ShaderError> {
    let (expected, b) = compose(s)?;
    if b != bytes
        || serde_json::to_vec(&expected).map_err(|e| ShaderError::Invalid(e.to_string()))?
            != serde_json::to_vec(a).map_err(|e| ShaderError::Invalid(e.to_string()))?
    {
        return Err(bad(
            "cached component differs from deterministic actual-source composition",
        ));
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct VertexPayload {
    key: String,
    program: TerrainProgram,
    spirv: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
struct VertexEnvelope {
    payload_sha256: String,
    payload: Vec<u8>,
}
pub fn load_composed_vertex(
    directory: &std::path::Path,
    s: &TerrainSources,
) -> Result<Option<(TerrainProgram, Vec<u8>)>, ShaderError> {
    let key = composed_vertex_key(s)?;
    let file = directory.join(format!("{key}.json"));
    let metadata = match std::fs::metadata(&file) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if metadata.len() > 32 * 1024 * 1024 {
        return Err(bad("component cache extent"));
    }
    let e: VertexEnvelope = serde_json::from_slice(&std::fs::read(file)?)
        .map_err(|e| ShaderError::Invalid(e.to_string()))?;
    if sha256(&e.payload) != e.payload_sha256 {
        return Err(bad("component cache checksum"));
    }
    let p: VertexPayload =
        serde_json::from_slice(&e.payload).map_err(|e| ShaderError::Invalid(e.to_string()))?;
    if p.key != key {
        return Err(bad("component source/static-domain key"));
    }
    validate_composed_vertex(s, &p.program, &p.spirv)?;
    Ok(Some((p.program, p.spirv)))
}
pub fn store_composed_vertex(
    directory: &std::path::Path,
    s: &TerrainSources,
    a: &TerrainProgram,
    bytes: &[u8],
) -> Result<(), ShaderError> {
    use std::io::Write;
    validate_composed_vertex(s, a, bytes)?;
    let key = composed_vertex_key(s)?;
    std::fs::create_dir_all(directory)?;
    let final_path = directory.join(format!("{key}.json"));
    if final_path.exists() {
        load_composed_vertex(directory, s)?.ok_or_else(|| bad("component cache disappeared"))?;
        return Ok(());
    }
    let payload = serde_json::to_vec(&VertexPayload {
        key: key.clone(),
        program: a.clone(),
        spirv: bytes.to_vec(),
    })
    .map_err(|e| ShaderError::Invalid(e.to_string()))?;
    let envelope = serde_json::to_vec(&VertexEnvelope {
        payload_sha256: sha256(&payload),
        payload,
    })
    .map_err(|e| ShaderError::Invalid(e.to_string()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| ShaderError::Invalid(e.to_string()))?
        .as_nanos();
    let temp = directory.join(format!(".{key}-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&envelope)?;
        file.sync_all()?;
        drop(file);
        match std::fs::hard_link(&temp, &final_path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                load_composed_vertex(directory, s)?
                    .ok_or_else(|| bad("component cache disappeared"))?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    })();
    let _ = std::fs::remove_file(temp);
    result
}

/// CPU address proof only: it does not pin an allocator slot or source authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawParameters {
    pub vertex_word_base: u32,
    pub index_word_base: u32,
    pub first_index: u32,
    pub generated_first_vertex: u32,
    pub domain_word_base: u32,
    pub source_base_vertex: i32,
    pub vertex_count: u32,
    pub generated_vertex_count: u32,
}
impl DrawParameters {
    pub fn uniform_bytes(&self) -> [u8; 32] {
        let values = [
            self.vertex_word_base,
            self.index_word_base,
            self.first_index,
            self.generated_first_vertex,
            self.domain_word_base,
            self.source_base_vertex as u32,
            self.vertex_count,
            self.generated_vertex_count,
        ];
        let mut b = [0; 32];
        for (n, v) in values.iter().enumerate() {
            b[n * 4..n * 4 + 4].copy_from_slice(&v.to_le_bytes())
        }
        b
    }
    pub fn validate_ranges(
        &self,
        vertices: &[u8],
        indices: &[u8],
        domains: &[u8],
    ) -> Result<(), ShaderError> {
        if [vertices.len(), indices.len(), domains.len()]
            .into_iter()
            .any(|n| n % 4 != 0)
            || self.generated_vertex_count == 0
            || self.generated_vertex_count % 3 != 0
            || self.vertex_count == 0
        {
            return Err(bad("raw28/Uint32/domain view or complete patch count"));
        }
        let checked = |base: u64, count: u64, len: usize| -> Result<(), ShaderError> {
            if base
                .checked_add(count)
                .is_none_or(|end| end > len as u64 / 4 || end > u32::MAX as u64)
            {
                Err(bad("whole-storage address/view/overflow"))
            } else {
                Ok(())
            }
        };
        checked(
            self.vertex_word_base as u64,
            7 * self.vertex_count as u64,
            vertices.len(),
        )?;
        let first = self.index_word_base as u64 + self.first_index as u64;
        checked(first, self.generated_vertex_count as u64, indices.len())?;
        checked(
            self.domain_word_base as u64,
            3 * self.generated_vertex_count as u64,
            domains.len(),
        )?;
        self.generated_first_vertex
            .checked_add(self.generated_vertex_count)
            .ok_or_else(|| bad("actual generated draw range overflow"))?;
        for n in 0..self.generated_vertex_count as u64 {
            let at = ((first + n) * 4) as usize;
            let value = u32::from_le_bytes(indices[at..at + 4].try_into().unwrap()) as i64
                + self.source_base_vertex as i64;
            if value < 0 || value >= self.vertex_count as i64 {
                return Err(bad("original indexed control point outside current mesh"));
            }
        }
        Ok(())
    }
}
