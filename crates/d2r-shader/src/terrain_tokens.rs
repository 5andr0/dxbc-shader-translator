//! Bounded original SM5 token decoder for the proved pure Terrain VS/DS.
//! This derives instructions from original bytes; it contains no embedded program.
use crate::{ShaderError, terrain::OriginalTerrainStage};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn bad(s: &str) -> ShaderError {
    ShaderError::Unsupported(format!("Terrain typed source: {s}"))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Node(usize),
    Literal(u32),
    Input { register: u32, lane: u8 },
    Constant { bank: u32, row: u32, lane: u8 },
    ControlPoint { point: u32, register: u32, lane: u8 },
    Domain(u8),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Add,
    Exp2,
    Mad,
    Move,
    Multiply,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    pub id: usize,
    pub source_instruction: usize,
    pub source_word_offset: usize,
    pub destination_lane: u8,
    pub op: Op,
    pub operands: Vec<Value>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub semantic: String,
    pub semantic_index: u32,
    pub system_value: u32,
    pub component_type: u32,
    pub register: u32,
    pub mask: u8,
    pub read_write_mask: u8,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub stage: OriginalTerrainStage,
    pub nodes: Vec<Node>,
    pub outputs: Vec<Vec<Value>>,
    pub inputs: Vec<Signature>,
    pub output_signature: Vec<Signature>,
    pub cb_vectors: BTreeMap<u32, u32>,
    pub global_flags: u32,
    pub source_value_instructions: usize,
    pub consumed_cb_lanes: BTreeSet<(u32, u32, u8)>,
}
#[derive(Clone, Debug)]
pub(crate) enum Index {
    Immediate(u32),
    Relative { offset: u32, value: Box<Operand> },
}
#[derive(Clone, Debug)]
pub(crate) struct Operand {
    pub reg: u8,
    pub dimension: u8,
    pub selection: u8,
    pub lanes: Vec<u8>,
    pub indices: Vec<Index>,
    pub literals: Vec<u32>,
}
#[derive(Clone, Debug)]
pub(crate) struct Instruction {
    pub op: u16,
    pub index: usize,
    pub offset: usize,
    pub words: Vec<u32>,
}

fn take(words: &[u32], at: &mut usize) -> Result<u32, ShaderError> {
    let v = *words.get(*at).ok_or_else(|| bad("truncated operand"))?;
    *at += 1;
    Ok(v)
}
pub(crate) fn operand(words: &[u32], at: &mut usize, depth: usize) -> Result<Operand, ShaderError> {
    if depth > 2 {
        return Err(bad("operand recursion"));
    }
    let token = take(words, at)?;
    if token >> 31 != 0 {
        return Err(bad("unreviewed operand extension/minimum precision"));
    }
    let dimension = (token & 3) as u8;
    let selection = ((token >> 2) & 3) as u8;
    let reg = ((token >> 12) & 255) as u8;
    let mut indices = vec![];
    for n in 0..((token >> 20) & 3) {
        match (token >> (22 + 3 * n)) & 7 {
            0 => indices.push(Index::Immediate(take(words, at)?)),
            2 | 3 => {
                let offset = if (token >> (22 + 3 * n)) & 7 == 3 {
                    take(words, at)?
                } else {
                    0
                };
                indices.push(Index::Relative {
                    offset,
                    value: Box::new(operand(words, at, depth + 1)?),
                });
            }
            _ => return Err(bad("unreviewed index representation")),
        }
    }
    let lanes = match dimension {
        0 => vec![],
        1 => vec![0, 0, 0, 0],
        2 => match selection {
            0 => (0..4)
                .filter(|n| token & (1 << (n + 4)) != 0)
                .map(|n| n as u8)
                .collect(),
            1 => (0..4).map(|n| ((token >> (4 + 2 * n)) & 3) as u8).collect(),
            2 => vec![((token >> 4) & 3) as u8; 4],
            _ => return Err(bad("component selector")),
        },
        _ => return Err(bad("component dimension")),
    };
    let mut literals = vec![];
    if reg == 4 {
        let count = match dimension {
            1 => 1,
            2 => 4,
            _ => return Err(bad("literal width")),
        };
        for _ in 0..count {
            literals.push(take(words, at)?);
        }
    }
    if reg == 5 {
        return Err(bad("float64 operand"));
    }
    Ok(Operand {
        reg,
        dimension,
        selection,
        lanes,
        indices,
        literals,
    })
}
fn words(b: &[u8]) -> Result<Vec<u32>, ShaderError> {
    if b.len() % 4 != 0 {
        return Err(bad("unaligned words"));
    }
    Ok(b.chunks_exact(4)
        .map(|v| u32::from_le_bytes(v.try_into().unwrap()))
        .collect())
}
pub(crate) fn chunks(b: &[u8]) -> Result<BTreeMap<[u8; 4], Vec<u8>>, ShaderError> {
    if b.len() < 32 || b.len() > 4 * 1024 * 1024 || b.get(..4) != Some(b"DXBC") {
        return Err(bad("DXBC header/extent"));
    }
    let h = words(b)?;
    if h[6] as usize != b.len() || h[7] > 32 || 8 + h[7] as usize > h.len() {
        return Err(bad("DXBC directory"));
    }
    let mut out = BTreeMap::new();
    let mut ranges = vec![];
    for n in 0..h[7] as usize {
        let at = h[8 + n] as usize;
        if at % 4 != 0
            || at < 32 + 4 * h[7] as usize
            || at.checked_add(8).is_none_or(|x| x > b.len())
        {
            return Err(bad("DXBC chunk offset"));
        }
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        let end = at
            .checked_add(8)
            .and_then(|x| x.checked_add(size))
            .ok_or_else(|| bad("DXBC size overflow"))?;
        if end > b.len() || ranges.iter().any(|&(a, z)| at < z && a < end) {
            return Err(bad("DXBC chunk overlap/extent"));
        }
        let tag: [u8; 4] = b[at..at + 4].try_into().unwrap();
        if out.insert(tag, b[at + 8..end].to_vec()).is_some() {
            return Err(bad("duplicate chunk"));
        }
        ranges.push((at, end));
    }
    Ok(out)
}
pub(crate) fn signature(b: &[u8]) -> Result<Vec<Signature>, ShaderError> {
    if b.len() < 8 {
        return Err(bad("signature header"));
    }
    let n = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
    if n > 32 || 8 + n * 24 > b.len() {
        return Err(bad("signature count"));
    }
    let mut out = vec![];
    for k in 0..n {
        let q = words(&b[8 + k * 24..8 + (k + 1) * 24])?;
        let at = q[0] as usize;
        let s = b.get(at..).ok_or_else(|| bad("semantic string"))?;
        let end = s
            .iter()
            .position(|v| *v == 0)
            .ok_or_else(|| bad("semantic terminator"))?;
        if end > 64 {
            return Err(bad("semantic string length"));
        }
        out.push(Signature {
            semantic: std::str::from_utf8(&s[..end])
                .map_err(|_| bad("semantic encoding"))?
                .into(),
            semantic_index: q[1],
            system_value: q[2],
            component_type: q[3],
            register: q[4],
            mask: q[5] as u8,
            read_write_mask: (q[5] >> 8) as u8,
        });
    }
    Ok(out)
}
pub(crate) fn instructions(
    stage: OriginalTerrainStage,
    b: &[u8],
) -> Result<(Vec<Instruction>, Vec<Signature>, Vec<Signature>), ShaderError> {
    let c = chunks(b)?;
    let code = c
        .get(b"SHEX")
        .or_else(|| c.get(b"SHDR"))
        .ok_or_else(|| bad("missing token code"))?;
    let w = words(code)?;
    if w.len() < 2 || w[0] != ((stage.token_type() << 16) | 0x50) || w[1] as usize != w.len() {
        return Err(bad("original SM5 stage/version"));
    }
    let mut out = vec![];
    let mut at = 2;
    while at < w.len() {
        let token = w[at];
        let encoded = ((token >> 24) & 31) as usize;
        let len = if encoded == 0 && (token & 255) == 53 {
            *w.get(at + 1).ok_or_else(|| bad("custom-data length"))? as usize
        } else {
            encoded
        };
        if len == 0 || at + len > w.len() || out.len() > 4096 {
            return Err(bad("unreviewed instruction length"));
        }
        out.push(Instruction {
            op: (token & 255) as u16,
            index: out.len(),
            offset: at,
            words: w[at..at + len].to_vec(),
        });
        at += len;
    }
    Ok((
        out,
        signature(c.get(b"ISGN").ok_or_else(|| bad("input signature"))?)?,
        signature(c.get(b"OSGN").ok_or_else(|| bad("output signature"))?)?,
    ))
}
fn imm_indices(o: &Operand) -> Result<Vec<u32>, ShaderError> {
    o.indices
        .iter()
        .map(|v| match v {
            Index::Immediate(n) => Ok(*n),
            _ => Err(bad("dynamic index in pure VS/DS")),
        })
        .collect()
}
fn read(
    o: &Operand,
    lane: u8,
    stage: OriginalTerrainStage,
    state: &BTreeMap<(u8, Vec<u32>, u8), Value>,
    cb: &mut BTreeSet<(u32, u32, u8)>,
) -> Result<Value, ShaderError> {
    let index = imm_indices(o)?;
    let selected = if o.dimension == 2 {
        *o.lanes
            .get(lane as usize)
            .ok_or_else(|| bad("source swizzle"))?
    } else if o.dimension == 1 {
        0
    } else {
        return Err(bad("scalar source shape"));
    };
    match o.reg {
        0 | 2 => state
            .get(&(o.reg, index, selected))
            .cloned()
            .ok_or_else(|| bad("undefined consumed temporary/output lane")),
        4 => Ok(Value::Literal(
            *o.literals
                .get(if o.dimension == 2 {
                    selected as usize
                } else {
                    0
                })
                .ok_or_else(|| bad("literal lane"))?,
        )),
        1 if stage == OriginalTerrainStage::Vertex
            && index.len() == 1
            && ((index[0] == 0 && selected < 4) || (index[0] == 1 && selected < 3)) =>
        {
            Ok(Value::Input {
                register: index[0],
                lane: selected,
            })
        }
        8 if stage == OriginalTerrainStage::Vertex
            && index.len() == 2
            && [0, 3].contains(&index[0]) =>
        {
            cb.insert((index[0], index[1], selected));
            Ok(Value::Constant {
                bank: index[0],
                row: index[1],
                lane: selected,
            })
        }
        25 if stage == OriginalTerrainStage::Domain
            && index.len() == 2
            && index[0] < 3
            && index[1] < 6 =>
        {
            Ok(Value::ControlPoint {
                point: index[0],
                register: index[1],
                lane: selected,
            })
        }
        28 if stage == OriginalTerrainStage::Domain && index.is_empty() && selected < 3 => {
            Ok(Value::Domain(selected))
        }
        _ => Err(bad("external/unknown pure source operand")),
    }
}
pub(crate) fn pure(stage: OriginalTerrainStage, b: &[u8]) -> Result<Program, ShaderError> {
    if !matches!(
        stage,
        OriginalTerrainStage::Vertex | OriginalTerrainStage::Domain
    ) {
        return Err(bad("pure stage identity"));
    }
    let (ins, inputs, output_signature) = instructions(stage, b)?;
    let mut state = BTreeMap::new();
    let mut nodes = vec![];
    let mut cb_vectors = BTreeMap::new();
    let mut cb = BTreeSet::new();
    let mut flags = None;
    let mut count = 0;
    let mut returned = false;
    for i in ins {
        if returned {
            return Err(bad("instructions after return"));
        }
        match i.op {
            106 => {
                if i.words.len() != 1 || flags.replace((i.words[0] >> 11) & 511).is_some() {
                    return Err(bad("global flags declaration"));
                }
            }
            89 => {
                let mut at = 1;
                let o = operand(&i.words, &mut at, 0)?;
                let q = imm_indices(&o)?;
                if stage != OriginalTerrainStage::Vertex
                    || i.words[0] & 0x00ffff00 != 0
                    || at != i.words.len()
                    || o.reg != 8
                    || q.len() != 2
                    || cb_vectors.insert(q[0], q[1]).is_some()
                {
                    return Err(bad("constant-buffer declaration"));
                }
            }
            95 | 101 | 103 | 104 | 147 | 149 => { /* Authenticated declarations also undergo full interface/source checks. */
            }
            62 => {
                if i.words.len() != 1 {
                    return Err(bad("return operands"));
                }
                returned = true;
            }
            _ => {
                if i.words[0] & 0x80ffff00 != 0 {
                    return Err(bad("unknown arithmetic/precision/control extension"));
                }
                let (op, arity) = match i.op {
                    0 => (Op::Add, 2),
                    25 => (Op::Exp2, 1),
                    50 => (Op::Mad, 3),
                    54 => (Op::Move, 1),
                    56 => (Op::Multiply, 2),
                    _ => return Err(bad("unreviewed pure opcode")),
                };
                let mut at = 1;
                let dest = operand(&i.words, &mut at, 0)?;
                let indices = imm_indices(&dest)?;
                if ![0, 2].contains(&dest.reg)
                    || dest.dimension != 2
                    || dest.selection != 0
                    || dest.lanes.is_empty()
                    || indices.len() != 1
                {
                    return Err(bad("pure destination"));
                }
                let mut args = vec![];
                for _ in 0..arity {
                    args.push(operand(&i.words, &mut at, 0)?)
                }
                if at != i.words.len() {
                    return Err(bad("opcode operand arity"));
                }
                let mut pending = vec![];
                for lane in dest.lanes {
                    let operands = args
                        .iter()
                        .map(|o| read(o, lane, stage, &state, &mut cb))
                        .collect::<Result<Vec<_>, _>>()?;
                    let id = nodes.len();
                    nodes.push(Node {
                        id,
                        source_instruction: i.index,
                        source_word_offset: i.offset,
                        destination_lane: lane,
                        op,
                        operands,
                    });
                    pending.push(((dest.reg, indices.clone(), lane), Value::Node(id)));
                }
                state.extend(pending);
                count += 1;
            }
        }
    }
    if !returned || flags != Some(1) {
        return Err(bad("source return/refactoring permission"));
    }
    let widths = [4, 4, 3, 3, 3, 4];
    let mut outputs = vec![];
    for (reg, width) in widths.into_iter().enumerate() {
        outputs.push(
            (0..width)
                .map(|lane| {
                    state
                        .get(&(2, vec![reg as u32], lane))
                        .cloned()
                        .ok_or_else(|| bad("undefined source output"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        );
    }
    if state.keys().filter(|(t, _, _)| *t == 2).count() != 21 {
        return Err(bad("extra output lane"));
    }
    for &(bank, row, lane) in &cb {
        if lane >= 4 || cb_vectors.get(&bank).is_none_or(|n| row >= *n) {
            return Err(bad("CB access outside declared source view"));
        }
    }
    Ok(Program {
        stage,
        nodes,
        outputs,
        inputs,
        output_signature,
        cb_vectors,
        global_flags: 1,
        source_value_instructions: count,
        consumed_cb_lanes: cb,
    })
}
