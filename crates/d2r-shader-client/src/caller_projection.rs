//! Caller-projection data definitions (copied from d2r-shader-iridia caller_projection.rs,
//! same serde form). Sealing and projection run in the translator; a capability here carries the
//! recipe the translator re-seals from and the values its seal produced.
use serde::{Deserialize, Serialize};
pub const CALLER_REVISION:&str="d2r-iridia-explicit-original-caller-results56-v3-dead-guard-emits";
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NullImageFixture {
    pub profile:String,pub original_dxbc_sha256:String,pub group:u32,pub binding:u32,
    pub original_register:String,pub api:String,pub resource_is_null:bool,
    pub dimension:String,pub format:String,pub array_layers:u32,pub mip_levels:u32,
    pub first_array_slice:u32,pub most_detailed_mip:u32,pub shader_component_mapping:u32,
    pub plane_slice:u32,pub resource_min_lod_clamp_bits:u32,
    pub accepted_host_provenance_sha256:String,pub rule_revision:String,
}
/// Borrow from the existing non-deserializable UnitWitness upload bytes.
/// This narrow compile-time gate is not a substitute for that witness's full
/// bank/cohort/current-frame/physical-resource lifetime checks at submission.
pub struct UnitConsumedBytes<'a>{pub t4:&'a [u8]}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(deny_unknown_fields)]
pub struct NullBufferFixture {
    pub profile:String,pub original_dxbc_sha256:String,pub group:u32,pub binding:u32,pub original_register:String,
    pub api:String,pub resource_is_null:bool,pub format:String,pub first_element:u64,pub num_elements:u32,
    pub structure_byte_stride:u32,pub flags:u32,pub shader_component_mapping:u32,pub host_provenance_sha256:String,pub revision:String,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CallerEvidence {
    pub revision:String,pub profile:String,pub original_dxbc_sha256:String,
    pub fixtures:Vec<NullImageFixture>,pub unit_consumed_t4:Option<[u8;4]>,
    pub null_buffers:Vec<NullBufferFixture>,
    pub grants_current_host_authority:bool,
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct ProjectedOperation {pub function:String,pub expression:usize,pub source_register:u32,pub source_kind:String,pub result_type:String,pub source_result_semantics:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct UnreadResource {pub resource_id:u32,pub register:u32,pub original_resource_sha256:String,pub reason:String}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct GuardEmitRemoval {pub function:String,pub removed_expression_handles:Vec<usize>,pub removed_query_handles:Vec<usize>,pub query_expressions_unchanged:bool}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq,Eq)]
pub struct CallerProjectionReport {pub revision:String,pub evidence:CallerEvidence,pub operations:Vec<ProjectedOperation>,pub unread_resources:Vec<UnreadResource>,pub dead_guard_emits:Vec<GuardEmitRemoval>,pub unchanged_control_flow:bool,pub unchanged_global_declarations:bool,pub complete_resource_projection:bool,pub grants_current_host_authority:bool}

use crate::rpc::{self, CapRecipe, Request, SealInfo};
use crate::{Error, Result, SourceShader};

/// Original caller capability. Sealed by the translator; carries its recipe and evidence.
#[derive(Clone, Debug)]
pub struct CallerCapability {
    recipe: CapRecipe,
    evidence: CallerEvidence,
}
fn sealed(recipe: &CapRecipe, info: SealInfo) -> std::result::Result<CallerEvidence, String> {
    info.evidence.ok_or_else(|| format!("PRETRANS1177 ERROR: seal of {recipe:?} returned no evidence"))
}
impl CallerCapability {
    pub fn seal(profile: &str, fixtures: Vec<NullImageFixture>, unit: Option<UnitConsumedBytes<'_>>) -> std::result::Result<Self, String> {
        let unit_t4 = match unit {
            Some(u) => Some(rpc::blob(u.t4)?),
            None => None,
        };
        let recipe = CapRecipe::Caller { profile: profile.into(), fixtures, unit_t4 };
        let info: SealInfo = rpc::call_s(Request::Seal { cap: recipe.clone() })?;
        Ok(Self { evidence: sealed(&recipe, info)?, recipe })
    }
    pub fn seal_waves(images: Vec<NullImageFixture>, buffers: Vec<NullBufferFixture>) -> std::result::Result<Self, String> {
        let recipe = CapRecipe::Waves { images, buffers };
        let info: SealInfo = rpc::call_s(Request::Seal { cap: recipe.clone() })?;
        Ok(Self { evidence: sealed(&recipe, info)?, recipe })
    }
    pub fn evidence(&self) -> &CallerEvidence {
        &self.evidence
    }
    pub fn recipe(&self) -> &CapRecipe {
        &self.recipe
    }
    fn projection_of(of: &CapRecipe, info: &SealInfo) -> Result<Self> {
        let evidence = info.evidence.clone().ok_or_else(|| Error::Io(format!("PRETRANS1177 ERROR: seal of {of:?} returned no projection evidence")))?;
        Ok(Self { recipe: CapRecipe::Projection { of: Box::new(of.clone()) }, evidence })
    }
}

/// The seal of a pair capability (`seal(vs, ps, ...) -> Result<Self>`).
fn seal_pair(recipe: CapRecipe) -> Result<(CapRecipe, SealInfo, CallerCapability)> {
    let info: SealInfo = rpc::call_e(Request::Seal { cap: recipe.clone() })?;
    let projection = CallerCapability::projection_of(&recipe, &info)?;
    Ok((recipe, info, projection))
}
fn missing(what: &str) -> Error {
    Error::Io(format!("PRETRANS1177 ERROR: seal returned no {what}"))
}

pub mod standard_map1129 {
    //! Constants of the map Standard authority (d2r-shader-iridia standard_map1129.rs); seal in the translator.
    use super::*;
    pub const PROFILE: &str = "map-standard-typed-null1129-v1";
    pub const RULE: &str = "map-standard-typed-null1129-v1";
    pub const RECEIPT_KEY: &str = "map_standard1129";
    pub const HOST: &str = "map-host-typed-null-descriptors-default-slots-v1";
    pub struct MapStandardCapability1129 {
        pub(crate) recipe: CapRecipe,
    }
    impl MapStandardCapability1129 {
        pub fn seal(vs: &SourceShader, ps: &SourceShader, null_images: &[u32]) -> Result<Self> {
            // Its projection and receipt are crate-private in process: the seal answers only Ok/Err.
            let recipe = CapRecipe::MapStandard { vs: rpc::src(vs)?, ps: rpc::src(ps)?, null_images: null_images.to_vec() };
            let _: SealInfo = rpc::call_e(Request::Seal { cap: recipe.clone() })?;
            Ok(Self { recipe })
        }
    }
}

macro_rules! map_null_capability {
    ($module:ident, $ty:ident, $variant:ident, $profile:literal, $rule:literal, $doc:literal) => {
        pub mod $module {
            #![doc = $doc]
            use super::*;
            pub const PROFILE: &str = $profile;
            pub const RULE: &str = $rule;
            pub const HOST: &str = super::standard_map1129::HOST;
            pub struct $ty {
                info: SealInfo,
                projection: CallerCapability,
            }
            impl $ty {
                pub fn seal(vs: &SourceShader, ps: &SourceShader, null_images: &[u32]) -> Result<Self> {
                    let (_, info, projection) = seal_pair(CapRecipe::$variant { vs: rpc::src(vs)?, ps: rpc::src(ps)?, null_images: null_images.to_vec() })?;
                    if info.null_images.is_none() || info.receipt.is_none() {
                        return Err(missing("null images / receipt"));
                    }
                    Ok(Self { info, projection })
                }
                pub fn projection(&self) -> &CallerCapability {
                    &self.projection
                }
                pub fn null_images(&self) -> Vec<u32> {
                    self.info.null_images.clone().unwrap_or_default()
                }
                pub fn receipt(&self) -> serde_json::Value {
                    self.info.receipt.clone().unwrap_or_default()
                }
            }
        }
    };
}
map_null_capability!(map_alpha_threshold1129, MapAlphaThresholdCapability1129, MapAlpha, "map-alpha-threshold-typed-null1129-v1", "map-alpha-threshold-typed-null1129-v1", "Constants of the map alpha-threshold authority (d2r-shader-iridia map_alpha_threshold1129.rs); seal in the translator.");
map_null_capability!(map_variant_body1153, MapVariantBodyCapability1153, MapVariant, "map-variant-body-typed-null1153-v1", "map-variant-body-typed-null1153-v1", "Constants of the map variant-body authority (d2r-shader-iridia map_variant_body1153.rs); seal in the translator.");

pub mod foliage_policy883 {
    //! The project's own Foliage23 policy constants and pure helpers (copied from d2r-shader-iridia
    //! foliage_policy883.rs); `Foliage23Capability` is sealed in the translator.
    use super::*;
    use crate::{SourceIdentity, Stage};
    pub const PROFILE: &str = "original-foliage23-source-host-v1";
    pub const RULE: &str = "foliage23-source-host-typed-null-srv-v1";
    /// Receipt key carried by both stages' caller contracts.
    pub const RECEIPT_KEY: &str = "foliage23";
    /// Reference binding set (typed-NULL table, grid, bank patch) this projection expects.
    pub const HOST: &str = "3d6ccac84867238809b74b988b5c1d2eeca48cad6306ecb5014fe540f66eeb69";
    /// `vs_models_v` key 1 (COLORS 1): meshes whose vertex type declares DiffuseColor0 (stride 24).
    pub const VS: &str = "73fc27a632d87206f46ad99562fd0bb8b93e337f18c91c0e67592673439cce5f";
    pub const VS_BYTES: u64 = 7800;
    /// `vs_models_v` key 0 (COLORS 0): meshes without a colour attribute (stride 20). The key
    /// follows the DiffuseColor0 attribute of the mesh vertex type. Its output signature is
    /// byte-identical to key 1, so the same foliage PS consumes it; it reads constant buffers
    /// b0 and b3 only (key 1 also reads b10), see [`vertex_constant_buffers`].
    pub const VS20: &str = "86f4e86f236d3c5ae512733c3d54e319f08e6c8c299057eebbb3b6708e262340";
    pub const VS20_BYTES: u64 = 1440;
    pub const PS: &str = "a3e7889bda18585cbc5ddb1f9fa7aecc21e63347816c27ddd19bff51ab49fdcf";
    pub const PS_BYTES: u64 = 60168;
    /// `ps_foliage_p` key 0 (ALPHA_BLEND 0): the opaque foliage colour pass (renderer colour
    /// pass, depth test EQUAL). Its declarations are key 1's without t42, so the same NULL table, grid and projection
    /// rule apply. Only the identity and the 34-entry caller differ.
    pub const PS0: &str = "ae50959313308aeceb858356263a59f96ccba34fb0dfd41a17fd86c730cac723";
    pub const PS0_BYTES: u64 = 60204;
    /// The admitted fragment identities (key 1 blended, key 0 opaque).
    pub fn admitted_pixel(sha: &str) -> bool {
        sha == PS || sha == PS0
    }
    /// Declared PS images this translation expects unbound (NULL).
    pub const NULLS: [u32; 12] = [30, 31, 32, 33, 39, 40, 41, 44, 45, 46, 47, 48];
    /// NULL images that are not read under the consumed t4 count.
    pub const UNOBSERVED: [u32; 5] = [30, 31, 32, 33, 40];
    /// Declared PS images bound to source textures.
    pub const PHYSICAL: [u32; 8] = [10, 11, 12, 13, 34, 35, 37, 42];
    /// Samplers read only through NULL images; they become unused projection dependencies.
    pub const DEAD_SAMPLERS: [u32; 4] = [3, 4, 6, 7];
    /// The renderer binds t39 to its directional shadow array (slice 0 is read under
    /// CB10[31].w, slice 1 under CB10[33].w). The other eleven NULL images stay NULL. In the
    /// program, s3 samples t39 and t41 and s6 samples t39 and t40, so with t39 live only s4 and s7
    /// stay unused projection dependencies. t40 keeps its unread proof (spot count
    /// `(t4 >> 16) & 255` is zero in every consumed t4).
    pub const RECEIVER_NULLS: [u32; 11] = [30, 31, 32, 33, 40, 41, 44, 45, 46, 47, 48];
    pub const RECEIVER_DEAD_SAMPLERS: [u32; 2] = [4, 7];
    /// The NULL image and dead sampler sets of a caller.
    pub fn null_sets(receiver: bool) -> (&'static [u32], &'static [u32]) {
        if receiver { (&RECEIVER_NULLS, &RECEIVER_DEAD_SAMPLERS) } else { (&NULLS, &DEAD_SAMPLERS) }
    }
    /// Maximum light-grid point records (<= 256, 48 bytes each).
    pub const MAX_POINTS: u64 = 256;
    /// Most unit lights a t4 count word encodes (`t4 >> 24`).
    pub const MAX_UNITS: u64 = 255;
    /// One t5 offset row (`dcl_resource_structured t5, 1024`): 256 u32 record offsets.
    pub const T5_ROW_BYTES: usize = 1024;
    /// Expected NULL descriptor dimension of a PS slot: (D3D12 view dimension, array layers).
    pub fn host_null_descriptor(slot: u32) -> (&'static str, u32) {
        match slot {
            33 | 34 => ("TextureCube", 0),
            39 | 40 => ("Texture2DArray", 1),
            45..=48 => ("Texture3D", 0),
            _ => ("Texture2D", 0),
        }
    }
    /// Matched-NULL or unobserved semantics of one projected source operation.
    pub fn operation(slot: u32, kind: &str) -> std::result::Result<&'static str, String> {
        match (slot, kind) {
            (30..=32, "gradient") | (33 | 40, "comparison-zero") => Ok("UnobservedUnderConsumedCount"),
            (39, "comparison-zero" | "exact") | (41 | 44..=48, "exact") => Ok("DefinedMatchedNullZero"),
            _ => Err(format!("Foliage23 unreviewed NULL operation t{slot} {kind}")),
        }
    }
    pub fn unread_reason(slot: u32) -> &'static str {
        if UNOBSERVED.contains(&slot) { "UnobservedSourceBranch" } else { "MatchedNull" }
    }
    /// The consumed light-grid t4 count must encode no ordinary, spot or unit light.
    pub fn consumed_t4(t4: &[u8]) -> std::result::Result<[u8; 4], String> {
        let bytes: [u8; 4] = t4.try_into().map_err(|_| "Foliage23 consumed t4 is one 4-byte element".to_string())?;
        let count = u32::from_le_bytes(bytes);
        if count & 63 != 0 || count >> 16 != 0 {
            return Err(format!("Foliage23 consumed t4 count {count:#010x} has ordinary/spot/unit lights"));
        }
        if u64::from(count >> 6) > MAX_POINTS {
            return Err("Foliage23 consumed t4 point count exceeds the host grid".into());
        }
        Ok(bytes)
    }
    /// One consumed unit record: its t5 list index, t8 base element and raw shadow-slot word.
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct UnitRecord {
        pub list_index: u32,
        pub t8_base: u32,
        pub shadow_slot_bits: u32,
    }
    /// Receipt of the consumed unit-light grid. The t4 count, the t5 offset row, the t8 extent and
    /// every unit record's shadow-slot word are sealed; the other record words (view-space
    /// positions, colour, range, contributions) are per-frame values the proof does not read.
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct UnitLights {
        pub t5_sha256: String,
        pub t8_elements: u64,
        pub records: Vec<UnitRecord>,
    }
    /// Unit records of the consumed single-cell grid: t5 offset row and t8 float4 records.
    pub fn consumed_units(t4: &[u8], t5: &[u8], t8: &[u8]) -> std::result::Result<([u8; 4], UnitLights), String> {
        let t4: [u8; 4] = t4.try_into().map_err(|_| "Foliage23 consumed t4 is one 4-byte element".to_string())?;
        let (points, units) = unit_count(&t4)?;
        if t5.len() != T5_ROW_BYTES || t8.is_empty() || t8.len() % 16 != 0 {
            return Err("Foliage23 consumed t5 row / t8 record extents".into());
        }
        let word = |b: &[u8], i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap());
        let t8_elements = (t8.len() / 16) as u64;
        let mut records = vec![];
        for i in 0..units {
            let list_index = points + i;
            let t8_base = word(t5, 4 * list_index as usize);
            let at = u64::from(t8_base) * 16 + 2 * 16 + 12;
            let shadow_slot_bits = if at + 4 <= t8.len() as u64 { word(t8, at as usize) } else { 0 };
            records.push(UnitRecord { list_index, t8_base, shadow_slot_bits });
        }
        check_records(points, units, t8_elements, &records)?;
        Ok((t4, UnitLights { t5_sha256: crate::sha256(t5), t8_elements, records }))
    }
    pub fn fixture(slot: u32) -> NullImageFixture {
        fixture_for(PS, slot)
    }
    /// The expected NULL descriptor of `slot` for the admitted PS `ps` (key 1 or key 0).
    pub fn fixture_for(ps: &str, slot: u32) -> NullImageFixture {
        let (dimension, array_layers) = host_null_descriptor(slot);
        NullImageFixture {
            profile: PROFILE.into(),
            original_dxbc_sha256: ps.into(),
            group: 1,
            binding: 32 + slot,
            original_register: format!("ps:t{slot}"),
            api: "D3D12-descriptor-table-null-SRV".into(),
            resource_is_null: true,
            dimension: dimension.into(),
            format: "R32G32B32A32_FLOAT".into(),
            array_layers,
            mip_levels: 1,
            first_array_slice: 0,
            most_detailed_mip: 0,
            shader_component_mapping: 5768,
            plane_slice: 0,
            resource_min_lod_clamp_bits: 0,
            accepted_host_provenance_sha256: HOST.into(),
            rule_revision: RULE.into(),
        }
    }
    pub fn fixtures() -> Vec<NullImageFixture> {
        NULLS.into_iter().map(fixture).collect()
    }
    /// The twelve expected NULL descriptors for the admitted PS `ps`.
    pub fn fixtures_for(ps: &str) -> Vec<NullImageFixture> {
        NULLS.into_iter().map(|s| fixture_for(ps, s)).collect()
    }
    /// The eleven expected NULL descriptors of the receiving caller.
    pub fn receiver_fixtures_for(ps: &str) -> Vec<NullImageFixture> {
        RECEIVER_NULLS.into_iter().map(|s| fixture_for(ps, s)).collect()
    }
    /// `Some(receiver)` when `fixtures` is exactly one admitted set.
    pub fn fixture_set(ps: &str, fixtures: &[NullImageFixture]) -> Option<bool> {
        if fixtures == fixtures_for(ps).as_slice() {
            Some(false)
        } else if fixtures == receiver_fixtures_for(ps).as_slice() {
            Some(true)
        } else {
            None
        }
    }
    /// The authenticated original stage content (DXBC hash, extent and role).
    pub fn source(s: &SourceIdentity) -> Result<()> {
        let known: &[(&str, u64)] = match s.stage {
            Stage::Vertex => &[(VS, VS_BYTES), (VS20, VS20_BYTES)],
            Stage::Fragment => &[(PS, PS_BYTES), (PS0, PS0_BYTES)], // + key 0
            _ => return Err(Error::Identity("Foliage23 stage".into())),
        };
        if !known.iter().any(|&(hash, bytes)| s.dxbc_sha256 == hash && s.byte_count == bytes) {
            return Err(Error::Identity("Foliage23 exact original stage identity required".into()));
        }
        Ok(())
    }
    /// The group-0 constant buffers of an admitted VS: key 1 reads b0, b3, b10; key 0 reads b0, b3.
    pub fn vertex_constant_buffers(vs_dxbc_sha256: &str) -> Result<&'static [u32]> {
        match vs_dxbc_sha256 {
            VS => Ok(&[0, 3, 10]),
            VS20 => Ok(&[0, 3]),
            _ => Err(Error::Identity("Foliage23 vertex stage not admitted".into())),
        }
    }
    /// `if_z r18.w` tests the raw slot word. A zero word, or a denormal one a float load may
    /// flush to zero, would sample t33; every other word keeps the unshadowed branch.
    fn slot_skips_t33(bits: u32) -> bool {
        bits & 0x7f80_0000 != 0
    }
    /// The consumed t4 count of a unit-light draw: no decal or spot light, at least one unit.
    fn unit_count(t4: &[u8; 4]) -> std::result::Result<(u32, u32), String> {
        let count = u32::from_le_bytes(*t4);
        if count & 63 != 0 || (count >> 16) & 255 != 0 {
            return Err(format!("Foliage23 consumed t4 count {count:#010x} has decal/spot lights"));
        }
        let (points, units) = ((count >> 6) & 1023, count >> 24);
        if u64::from(points) > MAX_POINTS || units == 0 || points + units > 256 {
            return Err(format!("Foliage23 consumed t4 count {count:#010x} point/unit range"));
        }
        Ok((points, units))
    }
    /// Each unit record listed after the point lights must keep t33 unobserved.
    fn check_records(points: u32, units: u32, t8_elements: u64, records: &[UnitRecord]) -> std::result::Result<(), String> {
        if records.len() != units as usize {
            return Err("Foliage23 unit record count differs from t4".into());
        }
        for (i, r) in records.iter().enumerate() {
            if r.list_index != points + i as u32 || u64::from(r.t8_base) + 4 > t8_elements {
                return Err(format!("Foliage23 unit record {i} lies outside the consumed t5/t8 views"));
            }
            if !slot_skips_t33(r.shadow_slot_bits) {
                return Err(format!("Foliage23 unit record {i} shadow slot {:#010x} would read the NULL t33 cube", r.shadow_slot_bits));
            }
        }
        Ok(())
    }

    pub struct Foliage23Capability {
        pub(crate) recipe: CapRecipe,
        info: SealInfo,
        projection: CallerCapability,
    }
    impl Foliage23Capability {
        pub fn seal(vs: &SourceShader, ps: &SourceShader, fixtures: Vec<NullImageFixture>, consumed_t4: &[u8]) -> Result<Self> {
            let t4 = rpc::blob(consumed_t4).map_err(Error::Io)?;
            Self::from_recipe(CapRecipe::Foliage { vs: rpc::src(vs)?, ps: rpc::src(ps)?, fixtures, t4, units: None })
        }
        pub fn seal_unit_lights(vs: &SourceShader, ps: &SourceShader, fixtures: Vec<NullImageFixture>,
            consumed_t4: &[u8], consumed_t5: &[u8], consumed_t8: &[u8]) -> Result<Self> {
            let b = |x: &[u8]| rpc::blob(x).map_err(Error::Io);
            Self::from_recipe(CapRecipe::Foliage { vs: rpc::src(vs)?, ps: rpc::src(ps)?, fixtures, t4: b(consumed_t4)?, units: Some((b(consumed_t5)?, b(consumed_t8)?)) })
        }
        fn from_recipe(recipe: CapRecipe) -> Result<Self> {
            let (recipe, info, projection) = seal_pair(recipe)?;
            if info.consumed_t4.is_none() || info.receipt.is_none() || info.receiver.is_none() {
                return Err(missing("consumed t4 / receipt / receiver"));
            }
            Ok(Self { recipe, info, projection })
        }
        pub fn projection(&self) -> &CallerCapability {
            &self.projection
        }
        pub fn consumed_t4(&self) -> [u8; 4] {
            self.info.consumed_t4.unwrap_or_default()
        }
        pub fn unit_lights(&self) -> Option<&UnitLights> {
            self.info.unit_lights.as_ref()
        }
        pub fn receipt(&self) -> serde_json::Value {
            self.info.receipt.clone().unwrap_or_default()
        }
        pub fn receiver(&self) -> bool {
            self.info.receiver.unwrap_or_default()
        }
    }
}
