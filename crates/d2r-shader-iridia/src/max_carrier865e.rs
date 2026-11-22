//! Precise source `max`. The controlled `Less(flush(src1), flush(src0))` is joined
//! by `compare_carrier865::join` (role `MaxCompare`); this module re-checks, in the actual imported
//! module, the complete WARP-measured selection graph around it.
use crate::compare_carrier865::stock_u32;
use crate::frac_carrier865d::uses;
use iridia::precision::{LESS_EXPONENT_MASK, LESS_SIGN_MASK, MAX_INFINITY, MAX_MAGNITUDE_MASK};
use naga::{Expression as E, Handle};

fn flush_select(m: &naga::Module, f: &naga::Function, sel: Handle<E>) -> Option<Handle<E>> {
    let E::Select { condition, accept, reject: bits } = f.expressions[sel] else { return None };
    let E::Binary { op: naga::BinaryOperator::Equal, left: e, right: zero } = f.expressions[condition] else { return None };
    let and = |h, mask| matches!(f.expressions[h], E::Binary { op: naga::BinaryOperator::And, left, right } if left == bits && stock_u32(m, f, right) == Some(mask));
    (and(e, LESS_EXPONENT_MASK) && and(accept, LESS_SIGN_MASK) && stock_u32(m, f, zero) == Some(0)).then_some(bits)
}

/// True when the imported controlled Less is exactly the admitted max graph's comparison.
pub(crate) fn stock_max_graph(m: &naga::Module, f: &naga::Function, less: Handle<E>) -> bool {
    let E::Binary { op: naga::BinaryOperator::Less, left: x1, right: x0 } = f.expressions[less] else { return false };
    let flushed = |x| match f.expressions[x] {
        E::As { expr, kind: naga::ScalarKind::Float, convert: None } => Some(expr),
        _ => None,
    };
    let (Some(s1), Some(s0)) = (flushed(x1), flushed(x0)) else { return false };
    let (Some(b1), Some(b0)) = (flush_select(m, f, s1), flush_select(m, f, s0)) else { return false };
    if uses(f, less) != 1 || uses(f, x0) != 1 || uses(f, x1) != 1 {
        return false;
    }
    let Some((pick, _)) = f.expressions.iter().find(|(_, e)| matches!(e, E::Select { condition, accept, reject } if *condition == less && *accept == s0 && *reject == s1)) else {
        return false;
    };
    if uses(f, pick) != 1 {
        return false;
    }
    let Some(nan1) = f.expressions.iter().find_map(|(_, e)| match *e {
        E::Select { condition, accept, reject } if reject == pick && accept == b0 => Some(condition),
        _ => None,
    }) else {
        return false;
    };
    matches!(f.expressions[nan1], E::Binary { op: naga::BinaryOperator::Greater, left, right }
        if stock_u32(m, f, right) == Some(MAX_INFINITY)
            && matches!(f.expressions[left], E::Binary { op: naga::BinaryOperator::And, left: b, right: mm } if b == b1 && stock_u32(m, f, mm) == Some(MAX_MAGNITUDE_MASK)))
}
