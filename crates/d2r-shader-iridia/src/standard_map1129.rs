//! The original Standard pair for the streamed map (a separately named authority).
//!
//! The authored-scene authority (`standard_policy668`) seals the Standard pair over the authored
//! scene's contract, closure, fixture banks and lights. The map renderer draws the
//! same original pair with its own inputs and never has those scene inputs. What the compiler
//! needs from the host is only the set of image slots the host binds as typed NULL descriptors:
//! the Standard630 projection zeroes exactly the samples of those slots.
//!
//! This authority is sealed from (1) the source pair and (2) the map host's typed-NULL image slot
//! set, and accepts it only when that set is exactly the Standard630 source-null set (the map binds
//! the six physical images `PHYSICAL` and a typed NULL descriptor on every other declared image,
//! the host renderer's default slots). It then runs the unchanged Standard630 projection
//! (`standard_policy668::project`) and the unchanged physical caller check
//! (`standard_policy668::contract`). It grants no authored-scene authority: the authored-scene
//! profile still refuses without its own sealed source.
use super::{CallerCapability, CallerEvidence, CallerProjectionReport, NullImageFixture, CALLER_REVISION};
use super::standard_policy668 as standard;
use crate::resource_contract::CallerContract;
use crate::{Error, PreparedShader, Result, SourceIdentity, SourceShader, Stage};
use serde_json::{json, Value};

pub const PROFILE: &str = "map-standard-typed-null1129-v1";
pub const RULE: &str = "map-standard-typed-null1129-v1";
/// Receipt key in the prepared caller contracts.
pub const RECEIPT_KEY: &str = "map_standard1129";
/// The host-provenance tag of the fixtures: the map host's typed-NULL descriptor rule.
pub const HOST: &str = "map-host-typed-null-descriptors-default-slots-v1";

fn need(b: bool, s: &str) -> Result<()> {
    if b { Ok(()) } else { Err(Error::Identity(format!("MapStandard1129: {s}"))) }
}
/// The host's NULL descriptor of one source-null slot (the same typed shapes as the
/// authored-scene fixtures: 33 cube, 39/40 2D array, 45..48 3D, others 2D).
fn fixture(ps_dxbc: &str, s: u32) -> NullImageFixture {
    NullImageFixture {
        profile: PROFILE.into(),
        original_dxbc_sha256: ps_dxbc.into(),
        group: 1,
        binding: 32 + s,
        original_register: format!("ps:t{s}"),
        api: "D3D12-descriptor-table-null-SRV".into(),
        resource_is_null: true,
        dimension: match s { 33 => "TextureCube", 39 | 40 => "Texture2DArray", 45..=48 => "Texture3D", _ => "Texture2D" }.into(),
        format: "R32G32B32A32_FLOAT".into(),
        array_layers: if matches!(s, 39 | 40) { 1 } else { 0 },
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
fn fixtures(ps_dxbc: &str) -> Vec<NullImageFixture> {
    standard::NULLS.into_iter().map(|s| fixture(ps_dxbc, s)).collect()
}
pub(crate) fn evidence(e: &CallerEvidence) -> Result<()> {
    need(
        e.revision == CALLER_REVISION && e.profile == PROFILE && e.fixtures == fixtures(&e.original_dxbc_sha256)
            && e.unit_consumed_t4.is_none() && e.null_buffers.is_empty() && !e.grants_current_host_authority,
        "exact map typed-NULL descriptor evidence",
    )
}

/// The map authority over one source pair.
pub struct MapStandardCapability1129 {
    vertex: SourceIdentity,
    fragment: SourceIdentity,
    projection: CallerCapability,
}
impl MapStandardCapability1129 {
    /// `null_images` = every PS image slot the map host binds as a typed NULL descriptor.
    pub fn seal(vs: &SourceShader, ps: &SourceShader, null_images: &[u32]) -> Result<Self> {
        need(vs.identity().stage == Stage::Vertex && ps.identity().stage == Stage::Fragment, "vertex/fragment source pair")?;
        let mut set = null_images.to_vec();
        set.sort_unstable();
        set.dedup();
        need(set.len() == null_images.len() && set == standard::NULLS, "host typed-NULL image set equals the Standard630 source-null set")?;
        let dxbc = ps.identity().dxbc_sha256.clone();
        let projection = CallerCapability {
            evidence: CallerEvidence { revision: CALLER_REVISION.into(), profile: PROFILE.into(), original_dxbc_sha256: dxbc.clone(), fixtures: fixtures(&dxbc), unit_consumed_t4: None, null_buffers: vec![], grants_current_host_authority: false },
            water649: None,
        };
        Ok(Self { vertex: vs.identity().clone(), fragment: ps.identity().clone(), projection })
    }
    pub(crate) fn projection(&self) -> &CallerCapability {
        &self.projection
    }
    pub(crate) fn recheck(&self, vs: &SourceShader, ps: &SourceShader) -> Result<()> {
        need(vs.identity() == &self.vertex && ps.identity() == &self.fragment, "same source pair")?;
        evidence(self.projection.evidence())
    }
    pub(crate) fn receipt(&self) -> Value {
        receipt(&self.vertex, &self.fragment)
    }
}
fn receipt(vertex: &SourceIdentity, fragment: &SourceIdentity) -> Value {
    json!({"profile": PROFILE, "rule": RULE, "host": HOST, "projection": standard::RULE,
        "null_images": standard::NULLS, "physical_images": standard::PHYSICAL,
        "vertex_dxbc": vertex.dxbc_sha256, "fragment_dxbc": fragment.dxbc_sha256,
        "authored_scene_authority": false, "grants_current_host_authority": false})
}

/// The receipt `prepare_graphics_impl104` writes under [`RECEIPT_KEY`] for this profile.
pub(crate) fn impl_receipt() -> Value {
    json!({"profile": PROFILE, "rule": RULE, "host": HOST, "projection": standard::RULE, "authored_scene_authority": false})
}
/// The unchanged Standard630 projection under this authority's evidence.
pub(crate) fn apply(p: &mut iridia::Program, cap: &CallerCapability) -> Result<CallerProjectionReport> {
    evidence(cap.evidence())?;
    standard::project(p, cap.evidence(), standard::Shape::Warn("MapStandard1129"))
}
/// The unchanged Standard668 physical caller recipe.
pub(crate) fn contract(stage: Stage, c: &CallerContract, projected: bool) -> Result<()> {
    standard::contract(stage, c, projected)
}
/// Cache revalidation of a prepared stage of this profile.
pub(crate) fn cached(p: &PreparedShader) -> Result<()> {
    if p.caller_contract()["profile"] != PROFILE {
        return need(p.caller_contract()[RECEIPT_KEY].is_null(), "map Standard receipt on another profile");
    }
    let r = &p.caller_contract()[RECEIPT_KEY];
    need(*r == impl_receipt() && p.identity().passes.get(RULE) == Some(&1) && p.payload().kind() == "controlled-spirv104", "map Standard cached receipt/passes/payload")?;
    contract(p.stage(), &p.resource_contract()?, p.stage() == Stage::Fragment)?;
    if p.stage() == Stage::Fragment {
        let report: CallerProjectionReport = serde_json::from_value(p.manifest()["caller_projection"].clone())?;
        evidence(&report.evidence)?;
        need(report.unread_resources.len() == 12 && report.unread_resources.iter().all(|u| standard::NULLS.contains(&u.register)), "cached source-null declaration coverage")?;
    }
    Ok(())
}
