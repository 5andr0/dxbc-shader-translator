//! Owner-thread, replaceable source-built vkd3d 2.1 compiler ABI.
//! The LGPL module is not statically linked or copied into this crate.
use crate::{ShaderError, sha256};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    marker::PhantomData,
    path::Path,
    ptr,
    rc::Rc,
};

pub const VKD3D_SOURCE_ARCHIVE_SHA256: &str =
    "7510146aff2adfb4ae07ab890701a607e5ff7c66e57100cfc9f630ec92eeda6a";

pub fn descriptor_remap_revision(profile: &OriginalProfile) -> &'static str {
    if crate::terrain::is_fragment(profile) {
        crate::terrain::REMAP_REVISION
    } else if let Some(r) = in_process_remap_revision(profile) {
        r
    } else {
        "vkd3d2.1-stage-set-cbv0-srv32-sampler64-uav96-v1"
    }
}
fn in_process_remap_revision(_: &OriginalProfile) -> Option<&'static str> {
    None
}
fn in_process_actor_layout(_: &OriginalProfile) -> bool {
    false
}
pub const COMPUTE_REMAP_REVISION: &str = "vkd3d2.1-compute-set2-cbv0-srv32-sampler64-uav96-v1";
fn validate_known_compute_profile(_: &OriginalComputeProfile) -> Result<(), ShaderError> {
    Err(ShaderError::Unsupported("compute profiles are registered only in the in-process build".into()))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ComputeStage {
    Compute,
}
/// A separate typed stage keeps existing infallible graphics-only profile APIs
/// unchanged. Unknown compute profiles are rejected before compiler execution.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OriginalComputeProfile {
    pub identity: String,
    pub stage: ComputeStage,
    pub path: String,
    pub resource_sha256: String,
    pub resource_bytes: usize,
    pub offset: usize,
    pub byte_count: usize,
    pub dxbc_sha256: String,
    pub descriptor_count: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Stage {
    Vertex,
    Fragment,
}
impl Stage {
    pub fn set(self) -> u32 {
        match self {
            Self::Vertex => 0,
            Self::Fragment => 1,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Vertex => "vs",
            Self::Fragment => "ps",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OriginalProfile {
    pub identity: String,
    pub stage: Stage,
    pub path: String,
    pub resource_sha256: String,
    pub resource_bytes: usize,
    pub offset: usize,
    pub byte_count: usize,
    pub dxbc_sha256: String,
    pub descriptor_count: u32,
}
pub fn selected_ice_profile(stage: Stage) -> OriginalProfile {
    let (path, source, bytes, offset, count, dxbc, descriptors) = match stage {
        Stage::Vertex => (
            "data/hd/shaders/vs_popcorn_uber_mesh_v.bin",
            "f0d42cec62e831f91be554efec73dd5b1cca5ddea8d51b0a57f9e94f724c8c88",
            11160780,
            8352384,
            3868,
            "2babfc57080b5033e58490aef43f90eb1952207599a55b2353b864f07b237cc0",
            4,
        ),
        Stage::Fragment => (
            "data/hd/shaders/ps_popcorn_uber_mesh_p.bin",
            "e5a8b7faba59e4adcc3f7fba58afc9cd8a6877853bee51cc8572aea30c323a2b",
            26001380,
            19259764,
            4560,
            "cb1a5b1b09ec5059f94b74173781dfff6b1092ccae8a872c09026c40696037bd",
            12,
        ),
    };
    OriginalProfile {
        identity: "ice-bolt-original-mesh-snapshot-profile1".into(),
        stage,
        path: path.into(),
        resource_sha256: source.into(),
        resource_bytes: bytes,
        offset,
        byte_count: count,
        dxbc_sha256: dxbc.into(),
        descriptor_count: descriptors,
    }
}
/// Game-version tolerance (`D2R_VERSION_TOLERANT=1`): when an original
/// shader resource differs from the pinned one (another D2R patch), find the pinned program
/// container anywhere in the installed resource; failing that, keep the container at the
/// pinned offset (the installed program). Every fallback prints a `VERSION WARN` line.
pub fn version_tolerant() -> bool {
    std::env::var_os("D2R_VERSION_TOLERANT").is_some_and(|v| !v.is_empty() && v != "0")
}
pub fn tolerant_dxbc<'a>(path: &str, resource: &'a [u8], offset: usize, byte_count: usize, want: &str) -> Option<&'a [u8]> {
    if !version_tolerant() {
        return None;
    }
    let container = |at: usize| -> Option<&'a [u8]> {
        let head = resource.get(at..at + 32)?;
        if &head[..4] != b"DXBC" { return None; }
        let n = u32::from_le_bytes(head[24..28].try_into().ok()?) as usize;
        resource.get(at..at.checked_add(n)?)
    };
    let mut at = 0;
    while let Some(i) = resource.get(at..).and_then(|r| r.windows(4).position(|w| w == b"DXBC")) {
        if let Some(c) = container(at + i).filter(|c| sha256(c) == want) {
            eprintln!("VERSION WARN: {path}: installed resource differs from the pinned one; pinned program {} found at offset {} (pinned offset {offset})", &want[..12.min(want.len())], at + i);
            return Some(c);
        }
        at += i + 4;
    }
    let c = container(offset).or_else(|| resource.get(offset..offset.checked_add(byte_count)?).filter(|c| c.starts_with(b"DXBC")))?;
    eprintln!("VERSION WARN: {path}: pinned program {} not in the installed resource; using the installed program at offset {offset} ({} bytes) - its output may differ", &want[..12.min(want.len())], c.len());
    Some(c)
}
pub fn extract_original(
    profile: &OriginalProfile,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, ShaderError>,
) -> Result<Vec<u8>, ShaderError> {
    let source = read(&profile.path)?;
    if source.len() != profile.resource_bytes || sha256(&source) != profile.resource_sha256 {
        if let Some(c) = tolerant_dxbc(&profile.path, &source, profile.offset, profile.byte_count, &profile.dxbc_sha256) {
            return Ok(c.to_vec());
        }
        return Err(ShaderError::Unsupported(
            "original shader resource version/hash".into(),
        ));
    }
    let end = profile
        .offset
        .checked_add(profile.byte_count)
        .ok_or_else(|| ShaderError::Invalid("original slice overflow".into()))?;
    let code = source
        .get(profile.offset..end)
        .ok_or_else(|| ShaderError::Invalid("original slice extent".into()))?;
    if code.len() < 32 || code.get(..4) != Some(b"DXBC") || sha256(code) != profile.dxbc_sha256 {
        return Err(ShaderError::Unsupported(
            "original permutation slice/hash".into(),
        ));
    }
    Ok(code.to_vec())
}
pub fn extract_compute_original(
    profile: &OriginalComputeProfile,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, ShaderError>,
) -> Result<Vec<u8>, ShaderError> {
    validate_known_compute_profile(profile)?;
    let source = read(&profile.path)?;
    if source.len() != profile.resource_bytes || sha256(&source) != profile.resource_sha256 {
        if let Some(c) = tolerant_dxbc(&profile.path, &source, profile.offset, profile.byte_count, &profile.dxbc_sha256) {
            return Ok(c.to_vec());
        }
        return Err(ShaderError::Unsupported(
            "original compute resource version/hash".into(),
        ));
    }
    let end = profile
        .offset
        .checked_add(profile.byte_count)
        .ok_or_else(|| ShaderError::Invalid("compute slice overflow".into()))?;
    let code = source
        .get(profile.offset..end)
        .ok_or_else(|| ShaderError::Invalid("compute slice extent".into()))?;
    if code.len() < 32 || code.get(..4) != Some(b"DXBC") || sha256(code) != profile.dxbc_sha256 {
        return Err(ShaderError::Unsupported(
            "original compute permutation/hash".into(),
        ));
    }
    Ok(code.to_vec())
}

struct TranslationRequest<'a> {
    identity: &'a str,
    stage_name: &'static str,
    set: u32,
    byte_count: usize,
    dxbc_sha256: &'a str,
    descriptor_count: u32,
    actor: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Code {
    code: *const c_void,
    size: usize,
}
#[repr(C)]
struct CompileInfo {
    kind: i32,
    next: *const c_void,
    source: Code,
    source_type: i32,
    target_type: i32,
    options: *const c_void,
    option_count: u32,
    log_level: i32,
    source_name: *const c_char,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Descriptor {
    kind: i32,
    space: u32,
    index: u32,
    resource_type: i32,
    data_type: i32,
    flags: u32,
    count: u32,
}
#[repr(C)]
struct ScanInfo {
    kind: i32,
    next: *const c_void,
    descriptors: *mut Descriptor,
    count: u32,
}
#[repr(C)]
struct Binding {
    kind: i32,
    space: u32,
    index: u32,
    visibility: i32,
    flags: u32,
    set: u32,
    binding: u32,
    count: u32,
}
#[repr(C)]
struct InterfaceInfo {
    kind: i32,
    next: *const c_void,
    bindings: *const Binding,
    count: u32,
    push: *const c_void,
    push_count: u32,
    combined: *const c_void,
    combined_count: u32,
    counters: *const c_void,
    counter_count: u32,
}
type Compile = unsafe extern "C" fn(*const CompileInfo, *mut Code, *mut *mut c_char) -> i32;
type Scan = unsafe extern "C" fn(*const CompileInfo, *mut *mut c_char) -> i32;
type FreeCode = unsafe extern "C" fn(*mut Code);
type FreeMessages = unsafe extern "C" fn(*mut c_char);
type FreeScan = unsafe extern "C" fn(*mut ScanInfo);
type GetVersion = unsafe extern "C" fn(*mut u32, *mut u32) -> *const c_char;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DescriptorBinding {
    pub class: i32,
    pub register: u32,
    pub resource_type: i32,
    pub data_type: i32,
    pub flags: u32,
    pub group: u32,
    pub binding: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompilerIdentity {
    pub module_sha256: String,
    pub version: String,
    pub declared_upstream_archive_sha256: String,
}
pub struct CompilerModule {
    _library: libloading::Library,
    compile: Compile,
    scan: Scan,
    free_code: FreeCode,
    free_messages: FreeMessages,
    free_scan: FreeScan,
    pub identity: CompilerIdentity,
    _owner_thread: PhantomData<Rc<()>>,
}
impl CompilerModule {
    /// Load only a caller-selected absolute locally source-built module. Loading it
    /// executes machine code; a matching API version does not establish trust.
    pub unsafe fn open(path: &Path) -> Result<Self, ShaderError> {
        if std::mem::size_of::<usize>() != 8 {
            return Err(ShaderError::Unsupported(
                "reviewed compiler ABI currently requires a 64-bit process".into(),
            ));
        }
        if !path.is_absolute() {
            return Err(ShaderError::Unsupported(
                "compiler module must be an explicit absolute path".into(),
            ));
        }
        let size = std::fs::metadata(path)?.len();
        if size < 16 || size > 64 * 1024 * 1024 {
            return Err(ShaderError::Invalid("compiler module extent".into()));
        }
        let module_sha = sha256(&std::fs::read(path)?);
        #[cfg(windows)]
        let library: libloading::Library =
            unsafe { libloading::os::windows::Library::load_with_flags(path, 0x100 | 0x800) }
                .map_err(|e| ShaderError::Unsupported(format!("compiler load: {e}")))?
                .into();
        #[cfg(not(windows))]
        let library: libloading::Library = unsafe {
            libloading::os::unix::Library::open(
                Some(path),
                libloading::os::unix::RTLD_NOW | libloading::os::unix::RTLD_LOCAL,
            )
        }
        .map_err(|e| ShaderError::Unsupported(format!("compiler load: {e}")))?
        .into();
        unsafe fn symbol<T: Copy>(l: &libloading::Library, name: &[u8]) -> Result<T, ShaderError> {
            unsafe { l.get::<T>(name) }
                .map(|s| *s)
                .map_err(|e| ShaderError::Unsupported(format!("compiler symbol: {e}")))
        }
        // SAFETY: all requested ABI entries are pinned to the reviewed public2.1 C header.
        let (compile, scan, free_code, free_messages, free_scan, get_version): (
            Compile,
            Scan,
            FreeCode,
            FreeMessages,
            FreeScan,
            GetVersion,
        ) = unsafe {
            (
                symbol(&library, b"vkd3d_shader_compile\0")?,
                symbol(&library, b"vkd3d_shader_scan\0")?,
                symbol(&library, b"vkd3d_shader_free_shader_code\0")?,
                symbol(&library, b"vkd3d_shader_free_messages\0")?,
                symbol(&library, b"vkd3d_shader_free_scan_descriptor_info\0")?,
                symbol(&library, b"vkd3d_shader_get_version\0")?,
            )
        };
        let (mut major, mut minor) = (0, 0);
        let text = unsafe { get_version(&mut major, &mut minor) };
        if major != 2 || minor != 1 || text.is_null() {
            return Err(ShaderError::Unsupported(
                "reviewed compiler ABI requires vkd3d2.1".into(),
            ));
        }
        let version = unsafe { CStr::from_ptr(text) }
            .to_string_lossy()
            .into_owned();
        Ok(Self {
            _library: library,
            compile,
            scan,
            free_code,
            free_messages,
            free_scan,
            identity: CompilerIdentity {
                module_sha256: module_sha,
                version,
                declared_upstream_archive_sha256: VKD3D_SOURCE_ARCHIVE_SHA256.into(),
            },
            _owner_thread: PhantomData,
        })
    }
    fn take_messages(&self, p: *mut c_char) -> String {
        if p.is_null() {
            return String::new();
        }
        let text = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
        unsafe { (self.free_messages)(p) };
        text
    }
    pub fn translate(
        &self,
        profile: &OriginalProfile,
        dxbc: &[u8],
    ) -> Result<(Vec<u8>, Vec<DescriptorBinding>, String), ShaderError> {
        self.translate_request(
            TranslationRequest {
                identity: &profile.identity,
                stage_name: profile.stage.name(),
                set: profile.stage.set(),
                byte_count: profile.byte_count,
                dxbc_sha256: &profile.dxbc_sha256,
                descriptor_count: profile.descriptor_count,
                actor: crate::terrain::is_fragment(profile) || in_process_actor_layout(profile),
            },
            dxbc,
        )
    }
    pub fn translate_compute(
        &self,
        profile: &OriginalComputeProfile,
        dxbc: &[u8],
    ) -> Result<(Vec<u8>, Vec<DescriptorBinding>, String), ShaderError> {
        validate_known_compute_profile(profile)?;
        self.translate_request(
            TranslationRequest {
                identity: &profile.identity,
                stage_name: "cs",
                set: 2,
                byte_count: profile.byte_count,
                dxbc_sha256: &profile.dxbc_sha256,
                descriptor_count: profile.descriptor_count,
                actor: false,
            },
            dxbc,
        )
    }
    fn translate_request(
        &self,
        request: TranslationRequest<'_>,
        dxbc: &[u8],
    ) -> Result<(Vec<u8>, Vec<DescriptorBinding>, String), ShaderError> {
        if dxbc.len() != request.byte_count || sha256(dxbc) != request.dxbc_sha256 {
            return Err(ShaderError::Unsupported(
                "compiler original input profile mismatch".into(),
            ));
        }
        let uri = CString::new(format!(
            "d2rvs://{}/{}.dxbc",
            request.identity, request.stage_name
        ))
        .map_err(|_| ShaderError::Invalid("source URI contains NUL".into()))?;
        let mut scan = ScanInfo {
            kind: 2,
            next: ptr::null(),
            descriptors: ptr::null_mut(),
            count: 0,
        };
        let mut info = CompileInfo {
            kind: 0,
            next: (&mut scan as *mut ScanInfo).cast(),
            source: Code {
                code: dxbc.as_ptr().cast(),
                size: dxbc.len(),
            },
            source_type: 1,
            target_type: 1,
            options: ptr::null(),
            option_count: 0,
            log_level: 3,
            source_name: uri.as_ptr(),
        };
        let mut log = ptr::null_mut();
        let status = unsafe { (self.scan)(&info, &mut log) };
        let mut messages = self.take_messages(log);
        struct ScanGuard {
            p: *mut ScanInfo,
            free: FreeScan,
        }
        impl Drop for ScanGuard {
            fn drop(&mut self) {
                unsafe { (self.free)(self.p) }
            }
        }
        let _guard = ScanGuard {
            p: &mut scan,
            free: self.free_scan,
        };
        if status < 0
            || scan.count != request.descriptor_count
            || scan.count > 128
            || scan.descriptors.is_null()
        {
            return Err(ShaderError::Unsupported(format!(
                "compiler declaration scan {status}: {messages}"
            )));
        }
        let descriptors =
            unsafe { std::slice::from_raw_parts(scan.descriptors, scan.count as usize) };
        let mut bindings = Vec::new();
        let mut metadata = Vec::new();
        let actor = request.actor;
        let mut occupied = std::collections::BTreeSet::new();
        for d in descriptors {
            let limit = if actor && matches!(d.kind, 0 | 1) {
                64
            } else {
                32
            };
            if d.space != 0 || d.count != 1 || d.index >= limit {
                return Err(ShaderError::Unsupported(
                    "resource arrays/space/index outside reviewed profile".into(),
                ));
            }
            let base = match d.kind {
                0 => 32,
                1 => {
                    if actor {
                        160
                    } else {
                        96
                    }
                }
                2 => 0,
                3 => {
                    if actor {
                        128
                    } else {
                        64
                    }
                }
                _ => return Err(ShaderError::Unsupported("descriptor class".into())),
            };
            if !occupied.insert((request.set, base + d.index)) {
                return Err(ShaderError::Unsupported(
                    "descriptor remap collision".into(),
                ));
            }
            let flags = if d.kind == 3 {
                0
            } else if d.resource_type == 1 || d.kind == 2 {
                1
            } else {
                2
            };
            bindings.push(Binding {
                kind: d.kind,
                space: d.space,
                index: d.index,
                visibility: 0,
                flags,
                set: request.set,
                binding: base + d.index,
                count: 1,
            });
            metadata.push(DescriptorBinding {
                class: d.kind,
                register: d.index,
                resource_type: d.resource_type,
                data_type: d.data_type,
                flags: d.flags,
                group: request.set,
                binding: base + d.index,
            });
        }
        let interface = InterfaceInfo {
            kind: 1,
            next: ptr::null(),
            bindings: bindings.as_ptr(),
            count: bindings.len() as u32,
            push: ptr::null(),
            push_count: 0,
            combined: ptr::null(),
            combined_count: 0,
            counters: ptr::null(),
            counter_count: 0,
        };
        info.next = (&interface as *const InterfaceInfo).cast();
        let mut output = Code {
            code: ptr::null(),
            size: 0,
        };
        log = ptr::null_mut();
        let status = unsafe { (self.compile)(&info, &mut output, &mut log) };
        messages.push_str(&self.take_messages(log));
        struct CodeGuard {
            p: *mut Code,
            free: FreeCode,
        }
        impl Drop for CodeGuard {
            fn drop(&mut self) {
                unsafe { (self.free)(self.p) }
            }
        }
        let _output_guard = CodeGuard {
            p: &mut output,
            free: self.free_code,
        };
        if status < 0
            || output.code.is_null()
            || output.size < 20
            || output.size > 4 * 1024 * 1024
            || output.size % 4 != 0
        {
            return Err(ShaderError::Unsupported(format!(
                "compiler translation {status}: {messages}"
            )));
        }
        Ok((
            unsafe { std::slice::from_raw_parts(output.code.cast::<u8>(), output.size) }.to_vec(),
            metadata,
            messages,
        ))
    }
}

/// Canonical emission scope: accepted-final-lowered SPIR-V -> WGSL only.
/// This does not attest the original DXBC -> lowered relationship; existing
/// family/profile/capability gates remain independently required.
pub(crate) struct FinalSpirvEmission {
    pub module: naga::Module,
    pub wgsl: String,
}
pub(crate) fn emit_final_spirv(lowered: &[u8]) -> Result<FinalSpirvEmission, ShaderError> {
    // Validate structural extents and supply missing integer coordinate types
    // only to Naga's frontend. Keep the caller's lowered bytes and their hashes
    // unchanged; this adds declarations, never source executable instructions.
    let (frontend_input, _) =
        crate::image_coordinate_types::ensure_integer_coordinate_types(lowered)?;
    // WGSL has no IsNan builtin. Preserve the predicate using its exact f32
    // exponent/mantissa bit test; original float math and control flow stay intact.
    let (frontend_input, _) = crate::isnan::lower_f32_isnan(&frontend_input)?;
    let options = naga::front::spv::Options {
        adjust_coordinate_space: false,
        strict_capabilities: true,
        block_ctx_dump_prefix: None,
    };
    let module = naga::front::spv::parse_u8_slice(&frontend_input, &options)
        .map_err(|e| ShaderError::Validation(format!("SPIR-V parse: {e:?}")))?;
    let info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| ShaderError::Validation(format!("Naga IR: {e:?}")))?;
    let wgsl = naga::back::wgsl::write_string(
        &module,
        &info,
        naga::back::wgsl::WriterFlags::EXPLICIT_TYPES,
    )
    .map_err(|e| ShaderError::Validation(format!("WGSL emit: {e:?}")))?;
    Ok(FinalSpirvEmission { module, wgsl })
}
