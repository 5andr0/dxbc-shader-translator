//! Shared CPU predicates of the Standard668 profile. No shader-value, device or upload authority.
pub const PROFILE:&str="original-standard630-flag2-live19-668-v1";
pub const RULE:&str="standard630-guarded-null-flag2-live19-v1";
pub const CONTRACT:&str="4de6d5adf4d411724c0dd898b163f946d2ac0b0b40e86bbee8edc53e1c40a781";
pub const VS:&str="86f4e86f236d3c5ae512733c3d54e319f08e6c8c299057eebbb3b6708e262340";
pub const PS:&str="7a396f77cfddbd13aec8f29a33bc1a3262a557fca37dcb8ada0fa97458b5fd1d";
pub const SHAPE:&str="sha256:ba7fe1104870d4bcdb9a58f7115f548dfb025875bf40f4fdcf36f282611d114a";
pub const NULLS:[u32;12]=[30,31,32,33,39,40,41,44,45,46,47,48];
pub const PHYSICAL:[u32;6]=[10,11,12,34,35,37];
pub fn operation(slot:u32,kind:&str)->Result<&'static str,String>{
    match(slot,kind){
        (30..=32,"gradient")|(33|39|40,"comparison-zero")|(39|45..=48,"exact")=>Ok("UnobservedUnderStandard630Guards"),
        (41|44,"exact")=>Ok("DefinedMatchedNullZero"),
        _=>Err("Standard668 unreviewed slot/operation; no comparison-zero result assumption".into()),
    }
}
pub fn operation_counts(slot:u32)->Option<[usize;3]>{ // exact, comparison, gradient
    Some(match slot{30|31=>[0,0,1],32=>[0,0,6],33|40=>[0,4,0],39=>[10,40,0],41|44..=48=>[1,0,0],_=>return None})
}
pub fn source(stage:u8,path:&str,parent:&str,bytes:u64,offset:u64,count:u64,hash:&str)->Result<(),String>{
    let(p,h,n,o,c,s)=match stage{
        0=>("data/hd/shaders/vs_models_v.bin","9cedad26a9cbdafad11f960a624842cd526914b0d612c5cca27fc3c463f5c555",31684,400,1440,VS),
        1=>("data/hd/shaders/ps_models_p.bin","af7daf1fd88d3e76460e2a22e497eedcd3e4a4e94299654b21bcf717196d958a",12012288,1360,57752,PS),
        _=>return Err("Standard668 unsupported original stage/family".into())};
    if (path,parent,bytes,offset,count,hash)!=(p,h,n,o,c,s){return Err("Standard668 exact original parent/slice/family identity required".into());}Ok(())
}
