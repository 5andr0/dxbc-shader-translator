//! The build part of the translator's cache id, from the inputs that decide what the
//! translator outputs, never from the program bytes.
//!
//! The cache folder used to be named after the SHA-256 of `d2r-shader-translator.exe`. The program
//! bytes also change with things that do not change any translation: the build folder (a
//! checkout embeds its own absolute path in one panic-location string, which shifts `.rdata`),
//! the `--remap-path-prefix` flags, codegen units. Every build in another folder then started
//! with an empty cache. This module hashes, path-independently:
//! - every file of every path package in the translator's `Cargo.lock` closure (the workspace
//!   crates and the `.patched` crates), except `target/`, `tests/`, `benches/`, `examples/` and
//!   dot entries;
//! - name, version, source and checksum of every registry or git package of that closure;
//! - `[workspace.package]` and the `[workspace.dependencies]` lines of the closure's packages
//!   (edition and features);
//! - `rustc -vV`, the target, debug assertions, target features and the rustflags without
//!   `--remap-path-prefix` (paths only).
//!
//! Left out on purpose: build folder and remaps, profile opt-level, codegen units, LTO and debug
//! info (they do not change what a Rust program computes). The run-time part of the id (the
//! translator's identity answer and D2R_VERSION_TOLERANT) is added by the
//! translator itself (`cache-id`).
//!
//! Shared by `build.rs` (through `#[path]`) and the translator's tests; std and sha2 only.
#![allow(dead_code)]

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const BUILD_SCHEMA: &str = "dxbc-shader-translator-build-v1";
/// The package whose closure is hashed.
pub const ROOT_PACKAGE: &str = "d2r-shader-translator";

pub fn sha256_hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

/// The toolchain facts that enter the id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toolchain {
    pub rustc_vv: String,
    pub target: String,
    pub debug_assertions: bool,
    pub target_features: String,
    /// Rustflags as passed (`CARGO_ENCODED_RUSTFLAGS` split at 0x1f).
    pub rustflags: Vec<String>,
}

/// Rustflags without the path remaps (`--remap-path-prefix=a=b` and `--remap-path-prefix a=b`).
pub fn output_rustflags(flags: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip_next = false;
    for f in flags {
        if skip_next {
            skip_next = false;
            continue;
        }
        if f == "--remap-path-prefix" {
            skip_next = true;
            continue;
        }
        if f.starts_with("--remap-path-prefix=") {
            continue;
        }
        out.push(f.clone());
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockPackage {
    pub name: String,
    pub version: String,
    pub source: Option<String>,
    pub checksum: Option<String>,
    pub dependencies: Vec<String>,
}

fn quoted(v: &str) -> Option<String> {
    let v = v.trim();
    v.strip_prefix('"').and_then(|v| v.strip_suffix('"')).map(str::to_owned)
}

/// The `[[package]]` entries of a Cargo.lock (format 3 and 4).
pub fn parse_lock(text: &str) -> Result<Vec<LockPackage>, String> {
    let mut out = Vec::new();
    let mut cur: Option<LockPackage> = None;
    let mut in_deps = false;
    let mut in_other_table = false;
    for line in text.lines() {
        let t = line.trim();
        if in_deps {
            if t == "]" {
                in_deps = false;
            } else if let Some(d) = t.strip_suffix(',').and_then(quoted).or_else(|| quoted(t)) {
                cur.as_mut().ok_or("dependency outside a package")?.dependencies.push(d);
            }
            continue;
        }
        if t == "[[package]]" {
            out.extend(cur.take());
            cur = Some(LockPackage { name: String::new(), version: String::new(), source: None, checksum: None, dependencies: Vec::new() });
            in_other_table = false;
            continue;
        }
        if t.starts_with('[') {
            out.extend(cur.take());
            in_other_table = true;
            continue;
        }
        if in_other_table {
            continue;
        }
        let Some(p) = cur.as_mut() else { continue };
        let Some((k, v)) = t.split_once('=') else { continue };
        match k.trim() {
            "name" => p.name = quoted(v).ok_or("lock name")?,
            "version" => p.version = quoted(v).ok_or("lock version")?,
            "source" => p.source = quoted(v),
            "checksum" => p.checksum = quoted(v),
            "dependencies" => {
                let v = v.trim();
                if v == "[" {
                    in_deps = true;
                } else if v.starts_with('[') && v.ends_with(']') {
                    for d in v[1..v.len() - 1].split(',') {
                        p.dependencies.extend(quoted(d));
                    }
                }
            }
            _ => {}
        }
    }
    out.extend(cur);
    if out.iter().any(|p| p.name.is_empty() || p.version.is_empty()) {
        return Err("Cargo.lock package without name or version".into());
    }
    Ok(out)
}

/// Resolves a lock dependency reference (`name`, `name version` or `name version (source)`).
fn resolve<'a>(lock: &'a [LockPackage], dep: &str) -> Result<&'a LockPackage, String> {
    let mut parts = dep.splitn(3, ' ');
    let name = parts.next().unwrap_or_default();
    let version = parts.next();
    let source = parts.next().map(|s| s.trim_start_matches('(').trim_end_matches(')'));
    let hits: Vec<&LockPackage> = lock
        .iter()
        .filter(|p| p.name == name && version.is_none_or(|v| p.version == v) && source.is_none_or(|s| p.source.as_deref() == Some(s)))
        .collect();
    match hits.as_slice() {
        [one] => Ok(one),
        [] => Err(format!("Cargo.lock: dependency {dep} not found")),
        _ => Err(format!("Cargo.lock: dependency {dep} is ambiguous")),
    }
}

/// The lock closure of `root` (all dependency kinds the lock records), sorted by name and version.
pub fn lock_closure<'a>(lock: &'a [LockPackage], root: &str) -> Result<Vec<&'a LockPackage>, String> {
    let start = lock.iter().filter(|p| p.name == root).collect::<Vec<_>>();
    let [start] = start.as_slice() else { return Err(format!("Cargo.lock: package {root} not found once")) };
    let mut seen: BTreeMap<(String, String, Option<String>), &LockPackage> = BTreeMap::new();
    let mut queue = vec![*start];
    while let Some(p) = queue.pop() {
        if seen.insert((p.name.clone(), p.version.clone(), p.source.clone()), p).is_some() {
            continue;
        }
        for d in &p.dependencies {
            queue.push(resolve(lock, d)?);
        }
    }
    Ok(seen.into_values().collect())
}

// ------------------------------------------------------------------------------- manifests --

/// The lines of one top-level TOML table (`[name]`), without comments and blank lines.
pub fn table_lines(manifest: &str, table: &str) -> Vec<String> {
    let header = format!("[{table}]");
    let mut inside = false;
    let mut out = Vec::new();
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == header;
            continue;
        }
        if inside && !t.is_empty() && !t.starts_with('#') {
            out.push(t.to_owned());
        }
    }
    out
}

/// `key = { ... path = "p" ... }` -> (key, p).
fn inline_path(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once('=')?;
    let i = v.find("path")?;
    let rest = v[i + 4..].trim_start().strip_prefix('=')?.trim_start().strip_prefix('"')?;
    Some((k.trim().trim_matches('"').to_owned(), rest[..rest.find('"')?].to_owned()))
}

fn dependency_table(header: &str) -> bool {
    let h = header.trim_matches(|c| c == '[' || c == ']');
    let last = h.rsplit('.').next().unwrap_or(h);
    matches!(last, "dependencies" | "build-dependencies" | "dev-dependencies")
}

/// Path dependencies named in a package manifest (every dependency kind and target table, inline
/// and `[dependencies.name]` forms, `workspace = true` through the workspace table). `true` = the
/// path is relative to the workspace root, else to the package.
pub fn path_dependencies(manifest: &str, workspace_deps: &BTreeMap<String, String>) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut table = String::new();
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            table = t.to_owned();
            continue;
        }
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if dependency_table(&table) {
            if let Some((_, p)) = inline_path(t) {
                out.push((p, false));
            } else if let Some((k, v)) = t.split_once('=') {
                let k = k.trim().split('.').next().unwrap_or_default().trim_matches('"');
                if v.contains("workspace") && v.contains("true") {
                    out.extend(workspace_deps.get(k).map(|p| (p.clone(), true)));
                }
            }
        } else {
            // `[dependencies.name]` table: its `path = "p"` line.
            let h = table.trim_matches(|c| c == '[' || c == ']');
            let mut segs = h.rsplitn(2, '.');
            let _name = segs.next();
            if segs.next().is_some_and(|parent| dependency_table(parent)) {
                if let Some(rest) = t.strip_prefix("path").map(str::trim_start).and_then(|r| r.strip_prefix('=')).and_then(|r| quoted(r.trim())) {
                    out.push((rest, false));
                }
            }
        }
    }
    out
}

/// `[package] name`.
pub fn package_name(manifest: &str) -> Option<String> {
    table_lines(manifest, "package").iter().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "name").then(|| quoted(v)).flatten()
    })
}

/// Lexical path normalisation (`a/b/../c` -> `a/c`), as Cargo does for path dependencies.
pub fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

/// Package name -> directory for every path package reachable from `start_dir`, plus the
/// `[patch.*]` path entries of the workspace manifest.
pub fn path_packages(root: &Path, start_dir: &Path) -> Result<BTreeMap<String, PathBuf>, String> {
    let ws = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("workspace Cargo.toml: {e}"))?;
    let mut workspace_deps = BTreeMap::new();
    for l in table_lines(&ws, "workspace.dependencies") {
        if let Some((k, p)) = inline_path(&l) {
            workspace_deps.insert(k, p);
        }
    }
    let mut out = BTreeMap::new();
    let mut queue = vec![normalize(start_dir)];
    let mut visited = BTreeSet::new();
    while let Some(dir) = queue.pop() {
        if !visited.insert(dir.clone()) {
            continue;
        }
        let m = std::fs::read_to_string(dir.join("Cargo.toml")).map_err(|e| format!("{}: {e}", dir.join("Cargo.toml").display()))?;
        let name = package_name(&m).ok_or_else(|| format!("{}: no [package] name", dir.display()))?;
        for (p, from_workspace) in path_dependencies(&m, &workspace_deps) {
            queue.push(normalize(&if from_workspace { root.join(p) } else { dir.join(p) }));
        }
        if let Some(old) = out.insert(name.clone(), dir.clone()) {
            if old != dir {
                return Err(format!("two path packages named {name}: {} and {}", old.display(), dir.display()));
            }
        }
    }
    for line in ws.lines().skip_while(|l| !l.trim().starts_with("[patch.")) {
        let t = line.trim();
        if t.starts_with('[') && !t.starts_with("[patch.") {
            break;
        }
        if let Some((k, p)) = inline_path(t) {
            let dir = normalize(&root.join(p));
            let name = std::fs::read_to_string(dir.join("Cargo.toml")).ok().and_then(|m| package_name(&m)).unwrap_or(k);
            out.insert(name, dir);
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------------------------- trees --

fn skipped(rel_depth: usize, name: &str) -> bool {
    name.starts_with('.') || name == "target" || (rel_depth == 0 && matches!(name, "tests" | "benches" | "examples"))
}

/// The bytes of one package file as they enter the id: rustc reads `.rs` sources with CRLF
/// normalised to LF (so a CRLF checkout and an LF edit compile the same), and Cargo parses
/// `Cargo.toml` as TOML (the test `hashed_folders_cover_every_source_rustc_read` checks that no
/// closure crate includes a Cargo.toml raw); every other file (Cargo.lock, JSON, WGSL) can enter
/// the output raw through `include_bytes!`/`include_str!` (d2r-shader-iridia hashes its Cargo.lock
/// into result metadata) and is hashed as it is.
pub fn line_endings_irrelevant(rel: &str) -> bool {
    rel.ends_with(".rs") || rel == "Cargo.toml" || rel.ends_with("/Cargo.toml")
}
pub fn file_bytes_for_id(rel: &str, bytes: Vec<u8>) -> Vec<u8> {
    if !line_endings_irrelevant(rel) || !bytes.contains(&b'\r') {
        return bytes;
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// SHA-256 over the sorted relative paths (`/`), lengths and bytes of a package folder.
pub fn tree_digest(dir: &Path) -> Result<(String, usize), String> {
    fn walk(base: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, PathBuf)>) -> Result<(), String> {
        for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let e = e.map_err(|e| format!("{}: {e}", dir.display()))?;
            let name = e.file_name().to_string_lossy().into_owned();
            if skipped(depth, &name) {
                continue;
            }
            let p = e.path();
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                walk(base, &p, depth + 1, out)?;
            } else {
                let rel = p.strip_prefix(base).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                out.push((rel, p));
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, 0, &mut files)?;
    files.sort();
    let mut h = Sha256::new();
    for (rel, p) in &files {
        let b = file_bytes_for_id(rel, std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?);
        h.update(rel.as_bytes());
        h.update([0]);
        h.update((b.len() as u64).to_le_bytes());
        h.update(&b);
    }
    Ok((h.finalize().iter().map(|x| format!("{x:02x}")).collect(), files.len()))
}

// ---------------------------------------------------------------------------------- digest --

/// The build part of the id and its input lines (sorted; no machine paths).
pub struct BuildInputs {
    pub digest: String,
    pub lines: Vec<String>,
    /// Folders and files whose change must re-run the build script.
    pub watched: Vec<PathBuf>,
    /// Package name -> folder of every hashed path package.
    pub path_dirs: BTreeMap<String, PathBuf>,
}

/// `root` = workspace root (holds Cargo.toml and Cargo.lock), `start_dir` = the translator package.
pub fn build_inputs(root: &Path, start_dir: &Path, tc: &Toolchain) -> Result<BuildInputs, String> {
    let lock_text = std::fs::read_to_string(root.join("Cargo.lock")).map_err(|e| format!("Cargo.lock: {e}"))?;
    let lock = parse_lock(&lock_text)?;
    let closure = lock_closure(&lock, ROOT_PACKAGE)?;
    let dirs = path_packages(root, start_dir)?;
    let ws = std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("workspace Cargo.toml: {e}"))?;
    let names: BTreeSet<&str> = closure.iter().map(|p| p.name.as_str()).collect();

    let mut lines = vec![format!("schema {BUILD_SCHEMA}")];
    lines.push(format!("rustc {}", tc.rustc_vv.trim().replace("\r\n", "\n").replace('\n', " | ")));
    lines.push(format!("target {}", tc.target));
    lines.push(format!("debug_assertions {}", tc.debug_assertions));
    lines.push(format!("target_features {}", tc.target_features));
    lines.push(format!("rustflags {}", output_rustflags(&tc.rustflags).join("\u{1f}")));
    for l in table_lines(&ws, "workspace.package") {
        lines.push(format!("workspace.package {l}"));
    }
    for l in table_lines(&ws, "workspace.dependencies") {
        let Some((k, v)) = l.split_once('=') else { continue };
        let k = k.trim().trim_matches('"');
        let renamed = v.split("package").nth(1).and_then(|r| r.trim_start().strip_prefix('=')).and_then(|r| r.trim_start().strip_prefix('"')).and_then(|r| r.split('"').next());
        if names.contains(k) || renamed.is_some_and(|r| names.contains(r)) {
            lines.push(format!("workspace.dependencies {l}"));
        }
    }
    let mut watched = vec![root.join("Cargo.toml"), root.join("Cargo.lock")];
    let mut path_dirs = BTreeMap::new();
    for p in &closure {
        match &p.source {
            Some(src) => lines.push(format!("package {} {} {} {}", p.name, p.version, src, p.checksum.as_deref().unwrap_or("-"))),
            None => {
                let dir = dirs.get(&p.name).ok_or_else(|| format!("path package {} {} has no known folder", p.name, p.version))?;
                let (d, n) = tree_digest(dir)?;
                lines.push(format!("package {} {} path {d} ({n} files)", p.name, p.version));
                watched.push(dir.clone());
                path_dirs.insert(p.name.clone(), dir.clone());
            }
        }
    }
    let digest = sha256_hex(lines.join("\n").as_bytes());
    Ok(BuildInputs { digest, lines, watched, path_dirs })
}

/// Reads the toolchain facts from a build script's environment.
pub fn toolchain_from_build_env() -> Toolchain {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let vv = std::process::Command::new(&rustc).arg("-vV").output().ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
    Toolchain {
        rustc_vv: vv.unwrap_or_else(|| panic!("transid1177: `{} -vV` failed", rustc.to_string_lossy())),
        target: std::env::var("TARGET").unwrap_or_default(),
        debug_assertions: std::env::var_os("CARGO_CFG_DEBUG_ASSERTIONS").is_some(),
        target_features: std::env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default(),
        rustflags: std::env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default().split('\u{1f}').filter(|s| !s.is_empty()).map(str::to_owned).collect(),
    }
}
