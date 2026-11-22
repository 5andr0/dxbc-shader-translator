//! The original VariantMaterial body pixel program (`ps_models_p` key 8, the
//! COLOR_VARIANT body permutation) for the streamed map, as a separately named authority.
//!
//! It is the structural rule of `map_alpha_threshold1129`, under its own name: key 8 declares the
//! Standard key-0 slot group plus t13 (MaskTexture), and no t35 / t41, so its typed-NULL set is the
//! Standard630 source-null set restricted to the slots it declares. The program reads PS CB2 rows
//! 0..17 (three colour transforms) and CB2[20].z (mask floor), which key 0 never reads; the cold
//! state's colour shift reaches the body only through this permutation.
//!
//! The capability is sealed from (1) the source pair and (2) the map host's typed-NULL image
//! slots. It pins no program hash (structural rule; another patch prepares as installed). It
//! requires
//! - every host NULL slot is in the Standard630 source-null set (`standard_policy668::NULLS`);
//! - the projection finds each of them declared as an image of its declared dimension;
//! - every image the PS declares inside that set is in the host set (nothing left half bound);
//! - the reads on those slots are the reviewed sample forms, replaced by the Microsoft NULL-SRV
//!   result (zero), exactly as the generic caller projection does for every other profile.
//!
//! No authored-scene, current-host or GPU authority is granted.
use super::{CallerCapability, CallerEvidence, NullImageFixture, CALLER_REVISION};
use super::standard_policy668 as standard;
use crate::{Error, Result, SourceIdentity, SourceShader, Stage};
use serde_json::{json, Value};

pub const PROFILE: &str = "map-variant-body-typed-null1153-v1";
pub const RULE: &str = "map-variant-body-typed-null1153-v1";
/// The host-provenance tag of the fixtures: the map host's typed-NULL descriptor rule (the same
/// rule as the map Standard authority).
pub const HOST: &str = super::standard_map1129::HOST;

fn need(b: bool, s: &str) -> Result<()> {
    if b { Ok(()) } else { Err(Error::Identity(format!("MapVariantBody1153: {s}"))) }
}
/// The host's NULL descriptor of one slot, of the slot's declared dimension (a matched NULL).
fn fixture(ps_dxbc: &str, s: u32) -> NullImageFixture {
    NullImageFixture {
        profile: PROFILE.into(),
        original_dxbc_sha256: ps_dxbc.into(),
        group: 1,
        binding: 32 + s,
        original_register: format!("ps:t{s}"),
        api: "D3D12-descriptor-table-null-SRV".into(),
        resource_is_null: true,
        dimension: match s { 33 => "TextureCube", 30..=32 | 39 | 40 => "Texture2DArray", 45..=48 => "Texture3D", _ => "Texture2D" }.into(),
        format: "R32G32B32A32_FLOAT".into(),
        array_layers: if matches!(s, 30..=32 | 39 | 40) { 1 } else { 0 },
        mip_levels: 1,
        first_array_slice: 0,
        most_detailed_mip: 0,
        shader_component_mapping: 5768,
        plane_slice: 0,
        resource_min_lod_clamp_bits: 0,
        accepted_host_provenance_sha256: crate::sha256(HOST.as_bytes()),
        rule_revision: RULE.into(),
    }
}
fn slots(e: &CallerEvidence) -> Vec<u32> {
    e.fixtures.iter().map(|f| f.binding.wrapping_sub(32)).collect()
}
pub(crate) fn evidence(e: &CallerEvidence) -> Result<()> {
    let s = slots(e);
    need(
        e.revision == CALLER_REVISION && e.profile == PROFILE && !s.is_empty()
            && s.windows(2).all(|w| w[0] < w[1]) && s.iter().all(|x| standard::NULLS.contains(x))
            && e.fixtures == s.iter().map(|&x| fixture(&e.original_dxbc_sha256, x)).collect::<Vec<_>>()
            && e.unit_consumed_t4.is_none() && e.null_buffers.is_empty() && !e.grants_current_host_authority,
        "exact map typed-NULL descriptor evidence",
    )
}
/// After the generic projection: every image the PS declares inside the Standard630 source-null
/// set is one of the host NULL slots (the projection itself already proved each host slot is
/// declared with its declared dimension).
pub(crate) fn coverage(p: &iridia::Program, e: &CallerEvidence) -> Result<()> {
    let host = slots(e);
    for r in &p.interface.resources {
        if let Some(d) = r.provenance.d3d {
            if d.class == iridia::D3dClass::ShaderResource && standard::NULLS.contains(&d.register) {
                need(host.contains(&d.register), &format!("declared t{} is neither bound nor in the host NULL set", d.register))?;
            }
        }
    }
    Ok(())
}

/// The map authority over one VariantMaterial body source pair.
pub struct MapVariantBodyCapability1153 {
    vertex: SourceIdentity,
    fragment: SourceIdentity,
    projection: CallerCapability,
}
impl MapVariantBodyCapability1153 {
    /// `null_images` = every PS image slot the map host binds as a typed NULL descriptor.
    pub fn seal(vs: &SourceShader, ps: &SourceShader, null_images: &[u32]) -> Result<Self> {
        need(vs.identity().stage == Stage::Vertex && ps.identity().stage == Stage::Fragment, "vertex/fragment source pair")?;
        let mut set = null_images.to_vec();
        set.sort_unstable();
        set.dedup();
        need(set.len() == null_images.len() && !set.is_empty() && set.iter().all(|s| standard::NULLS.contains(s)),
            "host typed-NULL image set is a subset of the Standard630 source-null set")?;
        let dxbc = ps.identity().dxbc_sha256.clone();
        let fixtures = set.iter().map(|&s| fixture(&dxbc, s)).collect();
        let projection = CallerCapability {
            evidence: CallerEvidence { revision: CALLER_REVISION.into(), profile: PROFILE.into(), original_dxbc_sha256: dxbc, fixtures, unit_consumed_t4: None, null_buffers: vec![], grants_current_host_authority: false },
            water649: None,
        };
        evidence(projection.evidence())?;
        Ok(Self { vertex: vs.identity().clone(), fragment: ps.identity().clone(), projection })
    }
    /// The fragment projection capability for `projected_caller_contract` and
    /// `prepare_controlled_graphics104(PROFILE, ..)`.
    pub fn projection(&self) -> &CallerCapability {
        &self.projection
    }
    /// The host NULL slots this capability was sealed with.
    pub fn null_images(&self) -> Vec<u32> {
        slots(self.projection.evidence())
    }
    pub fn receipt(&self) -> Value {
        json!({"profile": PROFILE, "rule": RULE, "host": HOST, "null_images": self.null_images(),
            "vertex_dxbc": self.vertex.dxbc_sha256, "fragment_dxbc": self.fragment.dxbc_sha256,
            "matched_null_rule": "Microsoft NULL-SRV reads of the declared dimension return zero",
            "authored_scene_authority": false, "grants_current_host_authority": false})
    }
}
