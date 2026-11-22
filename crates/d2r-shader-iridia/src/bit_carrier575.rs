//! Check bit-exact uint events against this writer execution's IDs and actual SPIR-V.
//! No floating operation or NoContraction waiver is admitted here.
use iridia::precision::{ProducerRole, reference};
use iridia_naga as ir;
#[derive(Clone, Debug)]
pub struct JoinedBits575 {
    pub source_word: usize,
    pub source: iridia::precision::OwnedExpression,
    pub result_id: u32,
    pub input_ids: Vec<u32>,
    pub opcode: u32,
    pub width: u8,
}
pub fn join(receipt:&iridia::precision::PrecisionReceipt, report:&ir::correspondence::EmissionReport, words:&[u32])->Result<Vec<JoinedBits575>,String>{
    let mut instructions=vec![];let mut at=5;
    if words.first()!=Some(&0x07230203){return Err("BitCarrierHeader".into())}
    while at<words.len(){let n=(words[at]>>16) as usize;if n==0 || at+n>words.len(){return Err("BitCarrierExtent".into())}instructions.push((words[at]&65535,&words[at+1..at+n]));at+=n;}
    let value=|r:ir::correspondence::ExpressionRef|->Result<u32,String>{
        let rows=report.values.iter().filter(|(s,_)|*s==r).collect::<Vec<_>>();
        if rows.len()!=1{return Err("BitCarrierValueMissingOrAmbiguous".into())}
        match rows[0].1 {ir::correspondence::EmittedValue::Value(id)=>Ok(id),_=>Err("BitCarrierNoValueId".into())}
    };
    let typed=|id:u32,width:u8|->bool{
        // Every candidate result must have a unique definition and an unsigned32 type.
        let defs=instructions.iter().filter(|(op,w)|matches!(*op,61|79|81|169|196) && w.len()>=2 && w[1]==id).collect::<Vec<_>>();
        if defs.len()!=1{return false}
        let mut ty=defs[0].1[0];
        if width>1 {let v=instructions.iter().filter(|(op,w)|*op==23 && w.len()==3 && w[0]==ty && w[2]==width as u32).collect::<Vec<_>>();if v.len()!=1{return false}ty=v[0].1[1];}
        instructions.iter().filter(|(op,w)|*op==21 && *w==[ty,32,0]).count()==1
    };
    let mut out=vec![];
    // A u32 zero constant (scalar, null, or composite/construct of zeros), by id.
    fn zero(instructions:&[(u32,&[u32])],id:u32,depth:u8)->bool{
        let defs=instructions.iter().filter(|(op,w)|matches!(*op,43|44|46|80) && w.len()>=2 && w[1]==id).collect::<Vec<_>>();
        if defs.len()!=1 || depth>2{return false}
        match defs[0].0 {43=>defs[0].1.len()==3 && defs[0].1[2]==0,46=>true,_=>defs[0].1.len()>2 && defs[0].1[2..].iter().all(|c|zero(instructions,*c,depth+1))}
    }
    for b in receipt.bindings().iter().filter(|b|matches!(b.role,ProducerRole::RegisterCopy|ProducerRole::LogicalShift|ProducerRole::ConditionalMove)){
        let id=value(reference(&b.producer))?;let inputs=b.operands.iter().map(|v|value(reference(v))).collect::<Result<Vec<_>,_>>()?;
        let defs=instructions.iter().filter(|(op,w)|matches!(*op,79|81|169|196) && w.len()>=2 && w[1]==id).collect::<Vec<_>>();
        if defs.len()!=1 || !typed(id,b.width.get()){return Err("BitCarrierProducerDefinitionOrType".into())}
        let (opcode,w)=*defs[0];
        let source=receipt.records().iter().find(|r|r.word==b.source_word).ok_or("BitCarrierSourceMissing")?;
        let valid=match b.role {
            ProducerRole::RegisterCopy=>{
                let lanes=iridia::precision::register_copy_lanes(source)?;
                inputs.len()==1 && typed(inputs[0],4) && match opcode {
                    79=>w.len()==4+lanes.len() && w[2..4]==[inputs[0],inputs[0]] && w[4..].iter().copied().eq(lanes.iter().map(|l|*l as u32)),
                    81=>lanes.len()==1 && w[2..]==[inputs[0],lanes[0] as u32],_=>false,
                }
            },
            ProducerRole::LogicalShift=>opcode==196 && inputs.len()==2 && w[2..]==inputs[..],
            // OpSelect(OpINotEqual(cond, 0u), src1, src2) on the joined u32 ids.
            ProducerRole::ConditionalMove=>{
                iridia::precision::conditional_move_lanes(source)?;
                opcode==169 && inputs.len()==3 && w.len()==5 && w[3..]==inputs[1..] && {
                    let c=instructions.iter().filter(|(op,x)|*op==171 && x.len()==4 && x[1]==w[2]).collect::<Vec<_>>();
                    c.len()==1 && c[0].1[2]==inputs[0] && zero(&instructions,c[0].1[3],0)
                }
            },
            _=>false,
        };
        if !valid{return Err("BitCarrierOpcodeOperandOrLaneMismatch".into())}
        out.push(JoinedBits575{source_word:b.source_word,source:b.producer.clone(),result_id:id,input_ids:inputs,opcode,width:b.width.get()});
    }
    Ok(out)
}
