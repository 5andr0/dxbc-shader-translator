//! Shared CPU predicates of the Water649 profile. No device, source-label or live-world authority.
pub const PROFILE: &str = "original-water630-current645-v1";
pub const RULE: &str = "water630-current645-null-source-v1";
pub const CONTRACT: &str = "4de6d5adf4d411724c0dd898b163f946d2ac0b0b40e86bbee8edc53e1c40a781";
pub const VS: &str = "d189aecafe708b4d120a71a7bec1527b662e159db49e761f7dc30e68fd07058a";
pub const PS: &str = "c3ac426052a491b82f909e73f22ef04f327c7dec1c7230dd614eff2ff3246f27";
pub const NULLS: [u32; 8] = [33,39,40,44,45,46,47,48];
/// Each hash must be calculated by the caller over the ACTUAL borrowed bytes.
/// This type alone is neither a source capability nor deserializable evidence.
#[derive(Clone,Copy)]
pub struct Pin<'a> { pub bytes: usize, pub sha: &'a str }
pub const INPUTS: [(usize, &str); 7] = [
    (664269, CONTRACT),
    (20933, "9296f5793c75a4d0eb41c589c691a4b3f08647b1066140adfd80e5b5eea067ca"),
    (43008, "c0052ac100818a4c0c806701d12b584b653756dd45c1c39ce5733a1991f283b8"),
    (4, "13785f70ee5f9ea2cbaeacbd60db44a3fa87001a1b874bbc50383e87ddbc27f6"),
    (1024, "013c5a7a623d2c20ed96494f9b965e38dcd620ca2068df09d29092c1151f47ff"),
    (304, "1c90d416b3630aa3c43e7c0c336d99599454a72a66dbedf70491aec367f0a01b"),
    (64, "013b154dc3672ac9bdf7108c845ddfc5634c6ebc71fffba73cb2b64e97729ee2"),
];
pub fn inputs(pins: &[Pin<'_>; 7]) -> Result<(), String> {
    if pins.iter().zip(INPUTS).any(|(p,(n,h))|p.bytes!=n||p.sha!=h) {
        return Err("Water649 actual shared630 source/header/bank/light extent or SHA differs".into());
    }
    Ok(())
}
pub fn null_slots(slots: &[u32]) -> Result<(), String> {
    if slots!=NULLS {return Err("Water649 complete canonical eight source-null image slots required".into());}
    Ok(())
}
pub fn grid(counts: &[u8], indices: &[u8], records: &[u8], unit: &[u8]) -> Result<(), String> {
    if counts!=0x01000140u32.to_le_bytes()||indices.len()!=1024||records.len()!=304||unit.len()!=64 {
        return Err("Water649 exact one-cell 5-point/1-unit grid,19 float4/304B required".into());
    }
    for (i,b) in indices.chunks_exact(4).enumerate() {
        if u32::from_le_bytes(b.try_into().unwrap())!=if i<6 {(3*i)as u32}else{0} {
            return Err("Water649 grid indices must be0,3,6,9,12,15 then zero".into());
        }
    }
    if records[240..]!=*unit {return Err("Water649 original unit tail differs".into());}
    Ok(())
}
pub fn stage_source(stage: u8, path: &str, resource_sha: &str, resource_bytes: u64,
    offset: u64, byte_count: u64, dxbc_sha: &str) -> Result<(), String> {
    let (want_path,n,h)=match stage {
        0=>("artifacts/synchronous-water/native05/vs_water_v.dxbc",15240,VS),
        1=>("artifacts/synchronous-water/native05/ps_water_p.dxbc",52204,PS),
        _=>return Err("Water649 stage must be original vertex or fragment".into()),
    };
    if path!=want_path||resource_sha!=h||resource_bytes!=n||offset!=0||byte_count!=n||dxbc_sha!=h {
        return Err("Water649 complete original resource/slice identity differs".into());
    }
    Ok(())
}

