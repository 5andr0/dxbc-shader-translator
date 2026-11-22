//! Read-only, fail-closed liveness audit for the source-inserted NULL load guards.
use iridia_naga as ir;
use ir::{Expression as E,Handle};
use std::collections::BTreeSet;

/// Remove only Emit bookkeeping for a source-matched guarded-load dependency
/// set. The expression arena, actual queries, other Emits and CFG stay intact.
/// A live dependency or a use outside the matched closure is unsupported.
pub fn remove_dead_guard_emits(f:&mut ir::Function,guards:&BTreeSet<usize>)->Result<Vec<usize>,String>{
    let live=live(f)?;
    if let Some(h)=guards.iter().find(|h|live.contains(h)){return Err(format!("source guard dependency {h} is live outside the projected NULL result"))}
    for (h,e) in f.expressions.iter(){if !guards.contains(&h.index())&&children(e)?.iter().any(|c|guards.contains(&c.index())){return Err(format!("source guard dependency escapes matched closure into expression{}",h.index()))}}
    let mut emitted_set=BTreeSet::new();emitted(&f.body,&mut emitted_set);
    let removed:Vec<_>=guards.intersection(&emitted_set).copied().collect();
    // Validation and proof above precede the first mutation.
    remove_emit_handles(&mut f.body,guards);Ok(removed)
}
fn remove_emit_handles(block:&mut ir::Block,handles:&BTreeSet<usize>){
    let old=std::mem::take(block);
    for (mut s,span)in old.span_into_iter(){
        if let ir::Statement::Emit(range)=s {
            let mut run=None;
            for h in range {if handles.contains(&h.index()){if let Some((a,b))=run.take(){block.push(ir::Statement::Emit(ir::Range::new_from_bounds(a,b)),span)}}else{run=Some((run.map_or(h,|r:(Handle<E>,Handle<E>)|r.0),h));}}
            if let Some((a,b))=run{block.push(ir::Statement::Emit(ir::Range::new_from_bounds(a,b)),span)}continue
        }
        match &mut s{ir::Statement::Block(b)=>remove_emit_handles(b,handles),ir::Statement::If{accept,reject,..}=>{remove_emit_handles(accept,handles);remove_emit_handles(reject,handles)},ir::Statement::Switch{cases,..}=>{for c in cases{remove_emit_handles(&mut c.body,handles)}},ir::Statement::Loop{body,continuing,..}=>{remove_emit_handles(body,handles);remove_emit_handles(continuing,handles)},_=>{}}
        block.push(s,span);
    }
}

pub fn children(e:&E)->Result<Vec<Handle<E>>,String>{
    let mut h=vec![];
    match e {
        E::Literal(_)|E::Constant(_)|E::Override(_)|E::ZeroValue(_)|E::FunctionArgument(_)|E::GlobalVariable(_)|E::LocalVariable(_)|E::CallResult(_)=>{},
        E::Compose{components,..}=>h.extend(components),
        E::Access{base,index}=>h.extend([base,index]),
        E::AccessIndex{base,..}=>h.push(*base),
        E::Splat{value,..}=>h.push(*value),
        E::Swizzle{vector,..}=>h.push(*vector),
        E::Load{pointer}=>h.push(*pointer),
        E::ImageSample{image,sampler,coordinate,array_index,offset,level,depth_ref,..}=>{
            h.extend([*image,*sampler,*coordinate]);h.extend(array_index);h.extend(offset);h.extend(depth_ref);
            match level{ir::SampleLevel::Auto|ir::SampleLevel::Zero=>{},ir::SampleLevel::Exact(x)|ir::SampleLevel::Bias(x)=>h.push(*x),ir::SampleLevel::Gradient{x,y}=>h.extend([*x,*y])}
        },
        E::ImageLoad{image,coordinate,array_index,sample,level}=>{h.extend([*image,*coordinate]);h.extend(array_index);h.extend(sample);h.extend(level);},
        E::ImageQuery{image,query}=>{h.push(*image);if let ir::ImageQuery::Size{level:Some(x)}=query{h.push(*x)}},
        E::Unary{expr,..}|E::Derivative{expr,..}|E::As{expr,..}=>h.push(*expr),
        E::Binary{left,right,..}=>h.extend([*left,*right]),
        E::Select{condition,accept,reject}=>h.extend([*condition,*accept,*reject]),
        E::Relational{argument,..}=>h.push(*argument),
        E::Math{arg,arg1,arg2,arg3,..}=>{h.push(*arg);h.extend(arg1);h.extend(arg2);h.extend(arg3)},
        E::ArrayLength(p)=>h.push(*p),
        _=>return Err(format!("unclassified source expression in liveness audit: {e:?}")),
    }
    Ok(h)
}
fn roots(block:&ir::Block,out:&mut BTreeSet<usize>)->Result<(),String>{
    for s in block {match s {
        ir::Statement::Emit(_)=>{},
        ir::Statement::Block(b)=>roots(b,out)?,
        ir::Statement::If{condition,accept,reject}=>{out.insert(condition.index());roots(accept,out)?;roots(reject,out)?},
        ir::Statement::Switch{selector,cases}=>{out.insert(selector.index());for c in cases{roots(&c.body,out)?}},
        ir::Statement::Loop{body,continuing,break_if}=>{roots(body,out)?;roots(continuing,out)?;out.extend(break_if.map(|h|h.index()));},
        ir::Statement::Return{value}=>out.extend(value.map(|h|h.index())),
        ir::Statement::Store{pointer,value}=>{out.extend([pointer.index(),value.index()]);},
        ir::Statement::Call{arguments,result,..}=>{out.extend(arguments.iter().map(|h|h.index()));out.extend(result.map(|h|h.index()));},
        ir::Statement::Break|ir::Statement::Continue|ir::Statement::Kill=>{},
        _=>return Err(format!("unclassified source statement in liveness audit: {s:?}")),
    }}Ok(())
}
pub fn live(f:&ir::Function)->Result<BTreeSet<usize>,String>{
    let mut out=BTreeSet::new();roots(&f.body,&mut out)?;
    for (_,v) in f.local_variables.iter(){out.extend(v.init.map(|h|h.index()));}
    for (h,e) in f.expressions.iter().rev(){if out.contains(&h.index()){for child in children(e)?{if child.index()>=h.index(){return Err("non-DAG expression dependency".into())}out.insert(child.index());}}}
    Ok(out)
}
pub fn emitted(block:&ir::Block,out:&mut BTreeSet<usize>){
    for s in block{match s{ir::Statement::Emit(r)=>out.extend(r.clone().map(|h|h.index())),ir::Statement::Block(b)=>emitted(b,out),ir::Statement::If{accept,reject,..}=>{emitted(accept,out);emitted(reject,out)},ir::Statement::Switch{cases,..}=>{for c in cases{emitted(&c.body,out)}},ir::Statement::Loop{body,continuing,..}=>{emitted(body,out);emitted(continuing,out)},_=>{}}}
}

