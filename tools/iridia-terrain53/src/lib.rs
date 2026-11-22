//! Public wrapper around the Terrain53 composer.
//! Preparation is not current-World, physical-resource or GPU admission.
pub mod terrain_iridia;

mod consumed_identity58 {
    include!(concat!(env!("OUT_DIR"), "/terrain_consumed_build58.rs"));
    pub fn identity() -> Option<(&'static str, &'static str)> {
        CONSUMED_BUILD_QUALIFIED.then_some((BUILD_IDENTITY_JSON, BUILD_IDENTITY_SHA256))
    }
}
pub fn require_consumed_identity58() -> Result<(&'static str, &'static str), &'static str> {
    consumed_identity58::identity().ok_or("Standalone Terrain53 source/lock is reference-only, not a consumed product build identity")
}
