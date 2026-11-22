//! Writes `OUT_DIR/translation_build1177.rs` with the build part of the cache id
//! (`TRANSLATION_BUILD1177`, 64 hex digits) and its input lines (`TRANSLATION_BUILD_LINES1177`).
//! See `src/transid1177.rs`.
#[path = "src/transid1177.rs"]
mod transid1177;

use std::path::PathBuf;

fn main() {
    let own = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = transid1177::normalize(&own.join("../.."));
    assert!(
        std::fs::read_to_string(root.join("Cargo.toml")).is_ok_and(|m| m.lines().any(|l| l.trim() == "[workspace]")),
        "transid1177: {} is not the workspace root",
        root.display()
    );
    let tc = transid1177::toolchain_from_build_env();
    let inputs = transid1177::build_inputs(&root, &own, &tc).unwrap_or_else(|e| panic!("transid1177: {e}"));
    for w in &inputs.watched {
        println!("cargo:rerun-if-changed={}", w.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("translation_build1177.rs");
    let text = format!(
        "pub const TRANSLATION_BUILD1177: &str = {:?};\npub const TRANSLATION_BUILD_LINES1177: &[&str] = &{:?};\n",
        inputs.digest,
        inputs.lines
    );
    std::fs::write(out, text).expect("transid1177: write OUT_DIR file");
}
