//! The standalone lock is not the resolved dependency graph of a path consumer.
//! Product builds embed the prebuild driver's verified actual dependency closure.
use crate::{Error,Result};
include!(concat!(env!("OUT_DIR"),"/consumed_build.rs"));
pub fn consumed_build_identity()->Option<&'static str>{CONSUMED_BUILD_QUALIFIED.then_some(BUILD_IDENTITY_JSON)}
pub fn consumed_build_identity_sha256()->Option<&'static str>{CONSUMED_BUILD_QUALIFIED.then_some(BUILD_IDENTITY_SHA256)}
pub fn require_consumed_build_identity()->Result<&'static str>{consumed_build_identity().ok_or_else(||Error::Identity("standalone reference build cannot grant product consumed-dependency admission".into()))}
pub(crate) fn identity()->(&'static str,&'static str){(if CONSUMED_BUILD_QUALIFIED{"consumed-cargo-closure-v2"}else{"standalone-lock-reference-only-v1"},BUILD_IDENTITY_SHA256)}
