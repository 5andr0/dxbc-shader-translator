//! Mechanical scalar typed source composition, with separately reported wrapper.
use crate::{
    ShaderError,
    lower::Instruction,
    terrain::{self, OriginalTerrainStage, TerrainSources, TypedTerrain},
    terrain_tokens::{Op, Value},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn bad(s: &str) -> ShaderError {
    ShaderError::Unsupported(format!("Terrain composition: {s}"))
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leaf {
    Node(usize),
    Literal(u32),
    ControlInput { point: u32, register: u32, lane: u8 },
    Constant { bank: u32, row: u32, lane: u8 },
    Domain(u8),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposedNode {
    pub id: usize,
    pub stage: OriginalTerrainStage,
    pub clone: Option<u32>,
    pub source_node: usize,
    pub source_instruction: usize,
    pub source_word_offset: usize,
    pub destination_lane: u8,
    pub op: Op,
    pub operands: Vec<Leaf>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: Vec<ComposedNode>,
    pub outputs: Vec<Vec<Leaf>>,
}
fn mapped(v: &Value, base: usize, point: u32) -> Result<Leaf, ShaderError> {
    Ok(match v {
        Value::Node(n) => Leaf::Node(base + n),
        Value::Literal(n) => Leaf::Literal(*n),
        Value::Input { register, lane } => Leaf::ControlInput {
            point,
            register: *register,
            lane: *lane,
        },
        Value::Constant { bank, row, lane } => Leaf::Constant {
            bank: *bank,
            row: *row,
            lane: *lane,
        },
        _ => return Err(bad("foreign VS source leaf")),
    })
}
pub(crate) fn graph(t: &TypedTerrain) -> Result<Graph, ShaderError> {
    let mut nodes = vec![];
    for point in 0..3 {
        let base = nodes.len();
        for n in &t.vertex.nodes {
            nodes.push(ComposedNode {
                id: base + n.id,
                stage: OriginalTerrainStage::Vertex,
                clone: Some(point),
                source_node: n.id,
                source_instruction: n.source_instruction,
                source_word_offset: n.source_word_offset,
                destination_lane: n.destination_lane,
                op: n.op,
                operands: n
                    .operands
                    .iter()
                    .map(|v| mapped(v, base, point))
                    .collect::<Result<_, _>>()?,
            });
        }
    }
    let ds_base = nodes.len();
    let dsmap = |v: &Value| -> Result<Leaf, ShaderError> {
        Ok(match v {
            Value::Node(n) => Leaf::Node(ds_base + n),
            Value::Literal(n) => Leaf::Literal(*n),
            Value::Domain(n) => Leaf::Domain(*n),
            Value::ControlPoint {
                point,
                register,
                lane,
            } => {
                let value = t
                    .vertex
                    .outputs
                    .get(*register as usize)
                    .and_then(|v| v.get(*lane as usize))
                    .ok_or_else(|| bad("DS source control-point lane"))?;
                mapped(value, *point as usize * t.vertex.nodes.len(), *point)?
            }
            _ => return Err(bad("foreign DS leaf")),
        })
    };
    for n in &t.domain.nodes {
        nodes.push(ComposedNode {
            id: ds_base + n.id,
            stage: OriginalTerrainStage::Domain,
            clone: None,
            source_node: n.id,
            source_instruction: n.source_instruction,
            source_word_offset: n.source_word_offset,
            destination_lane: n.destination_lane,
            op: n.op,
            operands: n.operands.iter().map(dsmap).collect::<Result<_, _>>()?,
        });
    }
    let outputs = t
        .domain
        .outputs
        .iter()
        .map(|v| v.iter().map(dsmap).collect::<Result<_, _>>())
        .collect::<Result<_, _>>()?;
    if nodes.len() != 402
        || nodes.iter().enumerate().any(|(n, v)| {
            v.id != n
                || v.operands
                    .iter()
                    .any(|a| matches!(a,Leaf::Node(id)if *id>=n))
        })
    {
        return Err(bad("complete402-node topological projection"));
    }
    Ok(Graph { nodes, outputs })
}
/// OUR wrapper contract, not a tessellator sample/cycle producer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeometryAbi {
    pub revision: String,
    pub group: u32,
    pub vertex_binding: u32,
    pub index_binding: u32,
    pub domain_binding: u32,
    pub draw_binding: u32,
    pub raw_vertex_stride_bytes: u32,
    pub index_stride_bytes: u32,
    pub explicit_domain_stride_bytes: u32,
    pub draw_uniform_bytes: u32,
    pub draw_fields: Vec<String>,
    pub source_patch_control_points: u32,
    pub generated_topology: String,
    pub native_domain_bits_or_cycle_proved: bool,
    pub consumer_current_range_and_last_use_required: bool,
}
impl GeometryAbi {
    pub fn exact() -> Self {
        Self {
            revision: "terrain-raw28-whole-storage-explicit-domain-nonindexed-v1".into(),
            group: 2,
            vertex_binding: 0,
            index_binding: 1,
            domain_binding: 2,
            draw_binding: 3,
            raw_vertex_stride_bytes: 28,
            index_stride_bytes: 4,
            explicit_domain_stride_bytes: 12,
            draw_uniform_bytes: 32,
            draw_fields: [
                "vertex_word_base",
                "index_word_base",
                "first_index",
                "generated_first_vertex",
                "domain_word_base",
                "source_base_vertex_twos_complement",
                "vertex_count",
                "generated_vertex_count",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            source_patch_control_points: 3,
            generated_topology:
                "TriangleList carrier; original PatchList3 identity retained separately".into(),
            native_domain_bits_or_cycle_proved: false,
            consumer_current_range_and_last_use_required: true,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Projection {
    pub node: usize,
    pub result_id: u32,
    pub opcode: u16,
    pub ext_opcode: Option<u32>,
    pub operand_ids: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainProgram {
    pub profile: String,
    pub revision: String,
    pub stages: Vec<terrain::StageProfile>,
    pub graph: Graph,
    pub projection: Vec<Projection>,
    pub geometry: GeometryAbi,
    pub spirv_sha256: String,
    pub wgsl_sha256: String,
    pub wgsl: String,
    pub layout: crate::reflection::ShaderLayout,
    pub source_uniform_views_required_bytes: Vec<(u32, u32, u64)>,
    pub refactoring_allowed: bool,
    pub native_numerical_or_raster_parity_proved: bool,
    pub automatic_native_domain_producer_supported: bool,
}

#[derive(Default)]
struct Builder {
    next: u32,
    types: BTreeMap<(u16, Vec<u32>), u32>,
    constants: BTreeMap<(u32, u32), u32>,
    decl: Vec<Instruction>,
    ann: Vec<Instruction>,
    debug: Vec<Instruction>,
    body: Vec<Instruction>,
}
impl Builder {
    fn id(&mut self) -> u32 {
        self.next += 1;
        self.next
    }
    fn ty(&mut self, op: u16, args: Vec<u32>) -> u32 {
        if let Some(n) = self.types.get(&(op, args.clone())) {
            return *n;
        }
        let n = self.id();
        let mut a = vec![n];
        a.extend(&args);
        self.decl.push(Instruction::new(op, a));
        self.types.insert((op, args), n);
        n
    }
    fn constant(&mut self, t: u32, bits: u32) -> u32 {
        if let Some(n) = self.constants.get(&(t, bits)) {
            return *n;
        }
        let n = self.id();
        self.decl.push(Instruction::new(43, vec![t, n, bits]));
        self.constants.insert((t, bits), n);
        n
    }
    fn deco(&mut self, id: u32, dec: u32, args: &[u32]) {
        let mut a = vec![id, dec];
        a.extend(args);
        self.ann.push(Instruction::new(71, a));
    }
    fn member(&mut self, id: u32, index: u32, dec: u32, args: &[u32]) {
        let mut a = vec![id, index, dec];
        a.extend(args);
        self.ann.push(Instruction::new(72, a));
    }
    fn variable(&mut self, t: u32, class: u32, name: &str) -> u32 {
        let p = self.ty(32, vec![class, t]);
        let id = self.id();
        self.decl.push(Instruction::new(59, vec![p, id, class]));
        let mut a = vec![id];
        a.extend(string(name));
        self.debug.push(Instruction::new(5, a));
        id
    }
    fn inst(&mut self, op: u16, t: u32, args: &[u32]) -> u32 {
        let n = self.id();
        let mut a = vec![t, n];
        a.extend(args);
        self.body.push(Instruction::new(op, a));
        n
    }
    fn load(&mut self, t: u32, p: u32) -> u32 {
        self.inst(61, t, &[p])
    }
    fn access(&mut self, ptr: u32, var: u32, index: &[u32]) -> u32 {
        let mut args = vec![var];
        args.extend(index);
        self.inst(65, ptr, &args)
    }
}
fn string(s: &str) -> Vec<u32> {
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    while b.len() % 4 != 0 {
        b.push(0)
    }
    b.chunks_exact(4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()))
        .collect()
}

struct Emitter {
    b: Builder,
    uint: u32,
    float: u32,
    ext: u32,
    scalar_uniform_u: u32,
    scalar_uniform_f: u32,
    vb: u32,
    ib: u32,
    domain: u32,
    uniforms: BTreeMap<u32, u32>,
    draw: u32,
}
impl Emitter {
    fn u(&mut self, n: u32) -> u32 {
        self.b.constant(self.uint, n)
    }
    fn binary(&mut self, op: u16, a: u32, c: u32) -> u32 {
        self.b.inst(op, self.uint, &[a, c])
    }
    fn storage(&mut self, var: u32, word: u32) -> u32 {
        let z = self.u(0);
        let p = self.b.access(self.scalar_uniform_u, var, &[z, word]);
        self.b.load(self.uint, p)
    }
    fn draw(&mut self, n: u32) -> u32 {
        let n = self.u(n);
        let p = self.b.access(self.scalar_uniform_u, self.draw, &[n]);
        self.b.load(self.uint, p)
    }
    fn fbits(&mut self, bits: u32) -> u32 {
        self.b.inst(124, self.float, &[bits])
    }
    fn uniform(&mut self, bank: u32, row: u32, lane: u8) -> Result<u32, ShaderError> {
        let var = *self
            .uniforms
            .get(&bank)
            .ok_or_else(|| bad("unknown original CB"))?;
        let z = self.u(0);
        let r = self.u(row);
        let l = self.u(lane as u32);
        let p = self.b.access(self.scalar_uniform_f, var, &[z, r, l]);
        Ok(self.b.load(self.float, p))
    }
}
fn resolve(
    e: &mut Emitter,
    leaf: &Leaf,
    results: &[u32],
    inputs: &BTreeMap<(u32, u32, u8), u32>,
    domain: &[u32; 3],
) -> Result<u32, ShaderError> {
    Ok(match leaf {
        Leaf::Node(n) => *results
            .get(*n)
            .ok_or_else(|| bad("SSA use before definition"))?,
        Leaf::Literal(bits) => e.b.constant(e.float, *bits),
        Leaf::ControlInput {
            point,
            register,
            lane,
        } => *inputs
            .get(&(*point, *register, *lane))
            .ok_or_else(|| bad("unreviewed control attribute"))?,
        Leaf::Constant { bank, row, lane } => e.uniform(*bank, *row, *lane)?,
        Leaf::Domain(n) => *domain.get(*n as usize).ok_or_else(|| bad("domain width"))?,
    })
}

pub fn compose(s: &TerrainSources) -> Result<(TerrainProgram, Vec<u8>), ShaderError> {
    let typed = terrain::inspect(s)?;
    let graph = graph(&typed)?;
    let mut b = Builder::default();
    let ext = b.id();
    let void = b.ty(19, vec![]);
    let uint = b.ty(21, vec![32, 0]);
    let float = b.ty(22, vec![32]);
    let vec4 = b.ty(23, vec![float, 4]);
    let function = b.ty(33, vec![void]);
    let ptr_u = b.ty(32, vec![2, uint]);
    let ptr_f = b.ty(32, vec![2, float]);
    let mut uniforms = BTreeMap::new();
    for (slot, count) in [(0, 4), (3, 33)] {
        let c = b.constant(uint, count);
        let a = b.ty(28, vec![vec4, c]);
        b.deco(a, 6, &[16]);
        let block = b.ty(30, vec![a]);
        b.deco(block, 2, &[]);
        b.member(block, 0, 35, &[0]);
        let v = b.variable(block, 2, &format!("terrain_original_cb{slot}"));
        b.deco(v, 34, &[0]);
        b.deco(v, 33, &[slot]);
        uniforms.insert(slot, v);
    }
    let array = b.ty(29, vec![uint]);
    b.deco(array, 6, &[4]);
    let storage = b.ty(30, vec![array]);
    b.deco(storage, 3, &[]);
    b.member(storage, 0, 35, &[0]);
    b.member(storage, 0, 24, &[]);
    let mut buffers = vec![];
    for binding in 0..3 {
        let v = b.variable(storage, 2, &format!("terrain_our_words{binding}"));
        b.deco(v, 34, &[2]);
        b.deco(v, 33, &[binding]);
        buffers.push(v);
    }
    let draw_type = b.ty(30, vec![uint; 8]);
    b.deco(draw_type, 2, &[]);
    for n in 0..8 {
        b.member(draw_type, n, 35, &[n * 4]);
    }
    let draw = b.variable(draw_type, 2, "terrain_our_draw");
    b.deco(draw, 34, &[2]);
    b.deco(draw, 33, &[3]);
    let vertex_index = b.variable(uint, 1, "terrain_generated_vertex_index");
    b.deco(vertex_index, 11, &[42]);
    let mut outputs = vec![];
    let mut output_types = vec![];
    for (register, width) in terrain::WIDTHS.into_iter().enumerate() {
        let ty = b.ty(23, vec![float, width as u32]);
        let var = b.variable(ty, 3, &format!("terrain_original_o{register}"));
        if register == 0 {
            b.deco(var, 11, &[0]);
        } else {
            b.deco(var, 30, &[register as u32]);
        }
        outputs.push(var);
        output_types.push(ty);
    }
    let main = b.id();
    let label = b.id();
    b.body
        .push(Instruction::new(54, vec![void, main, 0, function]));
    b.body.push(Instruction::new(248, vec![label]));
    let mut e = Emitter {
        b,
        uint,
        float,
        ext,
        scalar_uniform_u: ptr_u,
        scalar_uniform_f: ptr_f,
        vb: buffers[0],
        ib: buffers[1],
        domain: buffers[2],
        uniforms,
        draw,
    };
    let generated = e.b.load(uint, vertex_index);
    let first = e.draw(3);
    let local = e.binary(130, generated, first);
    let three = e.u(3);
    let patch = e.binary(134, local, three);
    let patch_start = e.binary(132, patch, three);
    let ib_base = e.draw(1);
    let first_index = e.draw(2);
    let ib_base = e.binary(128, ib_base, first_index);
    let ib_base = e.binary(128, ib_base, patch_start);
    let vb_base = e.draw(0);
    let source_base = e.draw(5);
    let seven = e.u(7);
    let mut inputs = BTreeMap::new();
    for point in 0..3 {
        let k = e.u(point);
        let word = e.binary(128, ib_base, k);
        let index = e.storage(e.ib, word);
        let index = e.binary(128, index, source_base);
        let vertex_word = e.binary(132, index, seven);
        let vertex_word = e.binary(128, vb_base, vertex_word);
        for lane in 0..7 {
            let l = e.u(lane);
            let word = e.binary(128, vertex_word, l);
            let bits = e.storage(e.vb, word);
            let value = e.fbits(bits);
            inputs.insert(
                (
                    point,
                    if lane < 4 { 0 } else { 1 },
                    if lane < 4 {
                        lane as u8
                    } else {
                        (lane - 4) as u8
                    },
                ),
                value,
            );
        }
    }
    let domain_base = e.draw(4);
    let domain_word = e.binary(132, local, three);
    let domain_word = e.binary(128, domain_base, domain_word);
    let mut domain = [0; 3];
    for lane in 0..3 {
        let l = e.u(lane);
        let word = e.binary(128, domain_word, l);
        let bits = e.storage(e.domain, word);
        domain[lane as usize] = e.fbits(bits);
    }
    let mut results = vec![];
    let mut projection = vec![];
    for n in &graph.nodes {
        let args = n
            .operands
            .iter()
            .map(|v| resolve(&mut e, v, &results, &inputs, &domain))
            .collect::<Result<Vec<_>, _>>()?;
        let (op, ext_op) = match n.op {
            Op::Add => (129, None),
            Op::Exp2 => (12, Some(29)),
            Op::Mad => (12, Some(50)),
            Op::Move => (83, None),
            Op::Multiply => (133, None),
        };
        let mut actual = vec![];
        if let Some(g) = ext_op {
            actual.extend([e.ext, g]);
        }
        actual.extend(&args);
        let id = e.b.inst(op, float, &actual);
        results.push(id);
        projection.push(Projection {
            node: n.id,
            result_id: id,
            opcode: op,
            ext_opcode: ext_op,
            operand_ids: args,
        });
    }
    for (register, row) in graph.outputs.iter().enumerate() {
        let values = row
            .iter()
            .map(|v| resolve(&mut e, v, &results, &inputs, &domain))
            .collect::<Result<Vec<_>, _>>()?;
        let composite = e.b.inst(80, output_types[register], &values);
        e.b.body
            .push(Instruction::new(62, vec![outputs[register], composite]));
    }
    e.b.body.push(Instruction::new(253, vec![]));
    e.b.body.push(Instruction::new(56, vec![]));
    let mut ins = vec![Instruction::new(17, vec![1])];
    let mut import = vec![ext];
    import.extend(string("GLSL.std.450"));
    ins.push(Instruction::new(11, import));
    ins.push(Instruction::new(14, vec![0, 1]));
    let mut entry = vec![0, main];
    entry.extend(string("main"));
    entry.push(vertex_index);
    entry.extend(outputs);
    ins.push(Instruction::new(15, entry));
    ins.extend(e.b.debug);
    ins.extend(e.b.ann);
    ins.extend(e.b.decl);
    ins.extend(e.b.body);
    let mut words = vec![0x07230203, 0x00010000, 0, e.b.next + 1, 0];
    for i in ins {
        i.emit(&mut words)
    }
    let spirv: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let canonical = crate::compiler::emit_final_spirv(&spirv)?;
    let layout = crate::reflection::reflect(&canonical.module)?;
    let artifact = TerrainProgram {
        profile: terrain::PROFILE.into(),
        revision: terrain::REVISION.into(),
        stages: typed.stages,
        graph,
        projection,
        geometry: GeometryAbi::exact(),
        spirv_sha256: crate::sha256(&spirv),
        wgsl_sha256: crate::sha256(canonical.wgsl.as_bytes()),
        wgsl: canonical.wgsl,
        layout,
        source_uniform_views_required_bytes: vec![(0, 0, 1536), (0, 3, 1536)],
        refactoring_allowed: typed.refactoring_allowed,
        native_numerical_or_raster_parity_proved: false,
        automatic_native_domain_producer_supported: false,
    };
    Ok((artifact, spirv))
}
