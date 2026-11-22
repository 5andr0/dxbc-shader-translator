//! Authored adapter policy. Original shader bytes and light-grid bytes are supplied at run time
//! and authenticated; none are stored in the repository.
//!
//! Original Foliage23 caller profile: the original models VS / foliage PS pair with an expected
//! typed-NULL descriptor table, a single-cell light grid and per-model bank roots. Twelve
//! declared PS images are expected unbound (t30-t33, t39-t41, t44-t48):
//!
//! - t39/t40 (Texture2DArray, one layer), t33 (TextureCube), t41/t44 (Texture2D) and t45-t48
//!   (Texture3D) are NULL descriptors of the declared dimension. Their operations are matched
//!   NULL reads: the Microsoft NULL-SRV rule returns zero for every lane, including sample_c
//!   (the NULL39 rule of the generic models profiles). The 40 t39 sample_c reads rely on this
//!   rule; no per-draw bank predicate gate is required, unlike the Standard668/Background677
//!   profiles, which prove their t39 reads unobserved.
//! - t30-t32 are declared Texture2DArray while the expected NULL view is Texture2D. A
//!   mismatched NULL read is not defined, so the projection requires every such read to be
//!   unobserved. All seven are inside the loop over `t4 & 63` (ordinary lights); t33 and t40
//!   are inside the unit (`t4 >> 24`) and spot (`(t4 >> 16) & 255`) loops. The grid count word
//!   is `pointCount << 6` in one 4-byte element, and out-of-range structured reads return zero,
//!   so a consumed t4 with zero low-six and upper-sixteen bits makes all of them unobserved.
//!   t33 and t40 are additionally matched NULL descriptors, but they are labelled by the
//!   stronger unobserved proof.
//! - Unit lights. Every placed foliage model component of the game data carries
//!   `lightMask 19` (scene lights 1 | unit light 2 | decals 16), and the PS gates its unit loop
//!   with `CB2[20].x & 2` and `t4 >> 24`. Inside that loop each unit record (4 float4 of t8 at
//!   the t5 offset listed after the point lights) samples t33 only `if_z` its shadow-slot word
//!   (float4[2].w); a nonzero slot word takes the unshadowed branch (factor 1.0).
//!   `Foliage23Capability::seal_unit_lights` therefore admits unit lights when the consumed t5
//!   row and t8 records give every unit a nonzero, non-denormal slot word inside the t8 view,
//!   which keeps t33 unobserved. Slot word 0 stays refused: the NULL cube would replace a
//!   shadow cube. Decals and spot lights stay zero.
//!
//! No replacement image, physical zero resource, live-world permit or GPU authority is granted.
use super::{CallerCapability, CallerEvidence, NullImageFixture, CALLER_REVISION};
use crate::resource_contract::{
    CallerContract, RegisterClass as C, ResourceContractKind as K, UnreadReason, RESOURCE_REVISION,
};
use crate::{Error, PreparedShader, Result, SourceIdentity, SourceShader, Stage};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

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

fn need(b: bool, s: &str) -> Result<()> {
    if b { Ok(()) } else { Err(Error::Unsupported(format!("Foliage23: {s}"))) }
}

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
pub(crate) fn evidence(e: &CallerEvidence) -> Result<()> {
    need(
        e.revision == CALLER_REVISION
            && e.profile == PROFILE
            && admitted_pixel(&e.original_dxbc_sha256) // key 1 or key 0
            && fixture_set(&e.original_dxbc_sha256, &e.fixtures).is_some() // + the receiving caller's set
            && e.unit_consumed_t4.is_none()
            && e.null_buffers.is_empty()
            && !e.grants_current_host_authority,
        "exact source-host NULL descriptors; no other caller capability",
    )
}

/// Sealed by the exact source pair, the twelve expected NULL descriptors and the borrowed t4 bytes
/// the renderer uploads for this draw. The t4 gate is compile-time evidence: the renderer must
/// still bind exactly these bytes as the 4-byte t4 view at submission.
pub struct Foliage23Capability {
    vertex: SourceIdentity,
    fragment: SourceIdentity,
    consumed_t4: [u8; 4],
    units: Option<(Arc<[u8]>, Arc<[u8]>, UnitLights)>,
    projection: CallerCapability,
    /// The caller binds t39 as its shadow array, not as a NULL image.
    receiver: bool,
}
impl Foliage23Capability {
    pub fn seal(vs: &SourceShader, ps: &SourceShader, fixtures: Vec<NullImageFixture>, consumed_t4: &[u8]) -> Result<Self> {
        let consumed_t4 = self::consumed_t4(consumed_t4).map_err(Error::Unsupported)?;
        Self::seal_impl(vs, ps, fixtures, consumed_t4, None)
    }
    /// Sealed like `seal`, for a draw whose consumed grid carries unit lights (original
    /// lightMask 19). The renderer must bind exactly this t4 count and t5 row, a t8 view of
    /// the same extent, and keep every unit record's shadow-slot word (see [`UnitLights`]).
    pub fn seal_unit_lights(vs: &SourceShader, ps: &SourceShader, fixtures: Vec<NullImageFixture>,
        consumed_t4: &[u8], consumed_t5: &[u8], consumed_t8: &[u8]) -> Result<Self> {
        let (t4, units) = consumed_units(consumed_t4, consumed_t5, consumed_t8).map_err(Error::Unsupported)?;
        Self::seal_impl(vs, ps, fixtures, t4, Some((Arc::from(consumed_t5), Arc::from(consumed_t8), units)))
    }
    fn seal_impl(vs: &SourceShader, ps: &SourceShader, fixtures: Vec<NullImageFixture>, consumed_t4: [u8; 4],
        units: Option<(Arc<[u8]>, Arc<[u8]>, UnitLights)>) -> Result<Self> {
        source(vs.identity())?;
        source(ps.identity())?;
        need(vs.identity().stage == Stage::Vertex && ps.identity().stage == Stage::Fragment, "exact original stage pair")?;
        let mut fixtures = fixtures;
        fixtures.sort_by_key(|f| f.binding);
        let ps_sha = ps.identity().dxbc_sha256.clone(); // key 1 or key 0
        for f in &fixtures {
            super::validate_fixture(PROFILE, &ps_sha, f).map_err(Error::Unsupported)?;
        }
        // The twelve-NULL set or the receiving eleven-NULL set.
        let receiver = fixture_set(&ps_sha, &fixtures).ok_or_else(|| Error::Unsupported("Foliage23: complete exact source-host NULL descriptor set".into()))?;
        let projection = CallerCapability {
            evidence: CallerEvidence {
                revision: CALLER_REVISION.into(),
                profile: PROFILE.into(),
                original_dxbc_sha256: ps_sha,
                fixtures,
                unit_consumed_t4: None,
                null_buffers: vec![],
                grants_current_host_authority: false,
            },
            water649: None,
        };
        Ok(Self { vertex: vs.identity().clone(), fragment: ps.identity().clone(), consumed_t4, units, projection, receiver })
    }
    pub fn projection(&self) -> &CallerCapability {
        &self.projection
    }
    pub fn consumed_t4(&self) -> [u8; 4] {
        self.consumed_t4
    }
    /// The consumed unit-light receipt, if this draw carries unit lights.
    pub fn unit_lights(&self) -> Option<&UnitLights> {
        self.units.as_ref().map(|u| &u.2)
    }
    pub(crate) fn recheck(&self, vs: &SourceShader, ps: &SourceShader) -> Result<()> {
        need(vs.identity() == &self.vertex && ps.identity() == &self.fragment, "same original source pair")?;
        match &self.units {
            None => { self::consumed_t4(&self.consumed_t4).map_err(Error::Unsupported)?; }
            Some((t5, t8, units)) => {
                let again = consumed_units(&self.consumed_t4, t5, t8).map_err(Error::Unsupported)?;
                need(again == (self.consumed_t4, units.clone()), "same consumed unit-light grid")?;
            }
        }
        evidence(self.projection.evidence())
    }
    /// The physical fragment caller's t8 view must be exactly the consumed unit-light records.
    pub(crate) fn check_caller(&self, physical_fragment: &CallerContract) -> Result<()> {
        contract_with(Stage::Fragment, physical_fragment, false, self.unit_lights().map(|u| u.t8_elements))
    }
    pub fn receipt(&self) -> Value {
        receipt(&self.vertex.dxbc_sha256, &self.fragment.dxbc_sha256, &self.consumed_t4, self.unit_lights(), self.receiver)
    }
    /// Whether the caller reads t39 (the shadow array).
    pub fn receiver(&self) -> bool {
        self.receiver
    }
}
fn receipt(vs: &str, ps: &str, t4: &[u8; 4], units: Option<&UnitLights>, receiver: bool) -> Value {
    let mut r = json!({"profile":PROFILE,"rule":RULE,"host_source_sha256":HOST,"vertex_dxbc_sha256":vs,"pixel_dxbc_sha256":ps,
        "consumed_t4_le":t4,"null_registers":null_sets(receiver).0,"unobserved_registers":UNOBSERVED,
        "unobserved_proof":"t30-t32 under t4&63, t33 under t4>>24, t40 under (t4>>16)&255; consumed t4 has none",
        "matched_null_rule":"Microsoft NULL-SRV reads, sample_c included, return zero",
        "grants_current_host_authority":false});
    if let Some(u) = units {
        r["unobserved_proof"] = json!("t30-t32 under t4&63 and t40 under (t4>>16)&255 (both zero in the consumed t4); t33 only if_z a unit record's shadow-slot word: every consumed unit record (t5 offsets after the point lights, 4 float4 of t8) has a nonzero, non-denormal slot word");
        r["unit_lights"] = serde_json::to_value(u).expect("unit receipt");
        r["unit_light_source"] = json!("original lightMask 19 on every placed foliage model component; PS unit loop gated by CB2[20].x & 2 and t4 >> 24");
    }
    if receiver {
        r["receiver1168"] = json!({"t39":"directional shadow array (map_shadows ShadowTargets895)","live_samplers":[3, 6],
            "rows":"PS CB10 rows 14..33, 48, 49 written per frame by the caller; zero rows skip both shadow branches"});
    }
    r
}

/// The physical original caller before projection, and after projection only the NULL images
/// and their dead sampler dependencies may become unread declarations.
pub(crate) fn contract(stage: Stage, c: &CallerContract, projected: bool) -> Result<()> {
    contract_with(stage, c, projected, None)
}
/// The vertex caller of the admitted VS `vs_dxbc_sha256` (its own group-0 constant buffers).
pub(crate) fn vertex_contract(vs_dxbc_sha256: &str, c: &CallerContract) -> Result<()> {
    contract_impl(Stage::Vertex, c, false, None, vertex_constant_buffers(vs_dxbc_sha256)?)
}
/// `unit_t8`: the exact t8 element count of a consumed unit-light grid; `None` keeps the
/// point-only light-grid layout (48-byte point records). A vertex stage here is VS key 1.
pub(crate) fn contract_with(stage: Stage, c: &CallerContract, projected: bool, unit_t8: Option<u64>) -> Result<()> {
    contract_impl(stage, c, projected, unit_t8, &[0, 3, 10])
}
fn contract_impl(stage: Stage, c: &CallerContract, projected: bool, unit_t8: Option<u64>, vertex_cb: &[u32]) -> Result<()> {
    let group = match stage {
        Stage::Vertex => 0,
        Stage::Fragment => 1,
        _ => return Err(Error::Identity("Foliage23 stage".into())),
    };
    let cb: &[u32] = if group == 0 { vertex_cb } else { &[0, 1, 2, 3, 6, 10] };
    let sam: &[u32] = if group == 0 { &[] } else { &[0, 1, 3, 4, 6, 7] };
    // The key-0 PS caller is the key-1 caller without t42 (34 entries).
    let no_t42 = !c.resources.iter().any(|r| r.class == C::ShaderResource && r.register == 42);
    need(c.revision == RESOURCE_REVISION && (if group == 0 { c.resources.len() == vertex_cb.len() } else { c.resources.len() == 35 || (c.resources.len() == 34 && no_t42) }), "complete original caller/revision")?;
    // A projected caller that keeps t39 as a texture is the receiving
    // caller; its NULL / dead-sampler sets come from `null_sets(true)`.
    let receiver = projected && group == 1 && c.resources.iter().any(|r| r.class == C::ShaderResource && r.register == 39 && matches!(r.resource, K::Texture { .. }));
    let (nulls, dead_samplers) = null_sets(receiver);
    let mut keys = BTreeSet::new();
    for r in &c.resources {
        let s = r.register;
        need(r.group == group && r.space == 0 && r.count == 1 && keys.insert((r.class, s)), "whole original caller namespace/uniqueness")?;
        let b = match r.class {
            C::ConstantBuffer => Some(s),
            C::ShaderResource => s.checked_add(32),
            C::Sampler => s.checked_add(96),
            _ => None,
        }
        .ok_or_else(|| Error::Identity("Foliage23 namespace".into()))?;
        let null = group == 1 && r.class == C::ShaderResource && nulls.contains(&s);
        let dead_sampler = group == 1 && r.class == C::Sampler && dead_samplers.contains(&s);
        let valid = if projected && (null || dead_sampler) {
            let want = if dead_sampler {
                UnreadReason::UnusedProjectionDependency
            } else if UNOBSERVED.contains(&s) {
                UnreadReason::UnobservedSourceBranch
            } else {
                UnreadReason::MatchedNull
            };
            matches!(&r.resource, K::UnreadDeclaration { reason, .. } if *reason == want)
        } else {
            match &r.resource {
                K::ConstantBuffer { bound_bytes } => r.class == C::ConstantBuffer && cb.contains(&s) && *bound_bytes == 1536,
                K::StructuredBuffer { stride_bytes, view } => {
                    group == 1 && r.class == C::ShaderResource && matches!(s, 4 | 5)
                        && *stride_bytes == if s == 4 { 4 } else { 1024 }
                        && view.source_first_element == 0 && view.source_num_elements == 1
                        && view.source_stride_bytes == u64::from(*stride_bytes)
                        && view.source_resource_bytes == u64::from(*stride_bytes)
                }
                // Light-grid point records: 48 bytes each, or 48 zero bytes when empty. A unit-light
                // grid binds exactly its consumed records (points 3 and units 4 float4 each).
                K::TypedBuffer { format, view } => {
                    group == 1 && r.class == C::ShaderResource && s == 8 && format == "R32G32B32A32_FLOAT"
                        && view.source_first_element == 0 && view.source_stride_bytes == 16
                        && match unit_t8 {
                            None => view.source_num_elements % 3 == 0 && (3..=3 * MAX_POINTS).contains(&view.source_num_elements),
                            Some(n) => view.source_num_elements == n && (4..=3 * MAX_POINTS + 4 * MAX_UNITS).contains(&n),
                        }
                        && view.source_resource_bytes == 16 * view.source_num_elements
                }
                K::Texture { dimension, arrayed, multisampled, sample } => {
                    group == 1 && r.class == C::ShaderResource && (null || PHYSICAL.contains(&s) || (receiver && s == 39))
                        && dimension == match s { 33 | 34 => "cube", 45..=48 => "d3", _ => "d2" }
                        && *arrayed == matches!(s, 30..=32 | 39 | 40)
                        && !*multisampled
                        && sample == if matches!(s, 33 | 39 | 40) { "depth" } else { "float" }
                }
                K::Sampler { comparison } => r.class == C::Sampler && sam.contains(&s) && *comparison == (s >= 6),
                _ => false,
            }
        };
        need(r.binding == b && valid, "original physical caller changed; no substituted view/grid/texture")?;
    }
    Ok(())
}

/// Revalidate a cached foliage stage from its own serialized evidence.
pub(crate) fn cached(p: &PreparedShader) -> Result<()> {
    let receipt_value = &p.caller_contract()[RECEIPT_KEY];
    if p.caller_contract()["profile"] != PROFILE {
        return need(receipt_value.is_null(), "Foliage23 receipt on another profile");
    }
    source(&p.identity().source)?;
    let t4: [u8; 4] = serde_json::from_value(receipt_value["consumed_t4_le"].clone())?;
    let units: Option<UnitLights> = match receipt_value.get("unit_lights") {
        None => { consumed_t4(&t4).map_err(Error::Identity)?; None }
        Some(u) => {
            let u: UnitLights = serde_json::from_value(u.clone())?;
            let (points, count) = unit_count(&t4).map_err(Error::Identity)?;
            check_records(points, count, u.t8_elements, &u.records).map_err(Error::Identity)?;
            Some(u)
        }
    };
    let vs: String = serde_json::from_value(receipt_value["vertex_dxbc_sha256"].clone())?;
    vertex_constant_buffers(&vs)?;
    let ps: String = serde_json::from_value(receipt_value["pixel_dxbc_sha256"].clone())?;
    need(admitted_pixel(&ps), "Foliage23 cached pixel receipt names an admitted PS")?;
    need(p.stage() != Stage::Fragment || p.identity().source.dxbc_sha256 == ps, "Foliage23 cached pixel receipt names its own stage")?;
    need(p.stage() != Stage::Vertex || p.identity().source.dxbc_sha256 == vs, "Foliage23 cached vertex receipt names its own stage")?;
    need(
        *receipt_value == receipt(&vs, &ps, &t4, units.as_ref(), receipt_value.get("receiver1168").is_some())
            && p.identity().passes.get(RULE) == Some(&1)
            && p.payload().kind() == "controlled-spirv104"
            && p.payload().source_sha256() == Some(p.identity().source.dxbc_sha256.as_str())
            && p.manifest()["draw"].is_null(),
        "controlled source/provenance/caller cache identity",
    )?;
    if p.stage() == Stage::Vertex {
        vertex_contract(&vs, &p.resource_contract()?)?;
    } else {
        contract_with(p.stage(), &p.resource_contract()?, true, units.map(|u| u.t8_elements))?;
    }
    if p.stage() == Stage::Fragment {
        let report: super::CallerProjectionReport = serde_json::from_value(p.manifest()["caller_projection"].clone())?;
        evidence(&report.evidence)?;
        let mut seen = BTreeSet::new();
        for op in &report.operations {
            let semantics = operation(op.source_register, &op.source_kind).map_err(Error::Identity)?;
            need(op.source_result_semantics == semantics && seen.insert((&op.function, op.expression)), "cached source operation/semantics coverage")?;
        }
        let (nulls, _) = null_sets(fixture_set(&ps, &report.evidence.fixtures) == Some(true));
        need(
            report.unread_resources.len() == nulls.len()
                && report.unread_resources.iter().all(|r| nulls.contains(&r.register) && r.reason == unread_reason(r.register)),
            "cached source-null declaration coverage/reasons",
        )?;
    }
    Ok(())
}

