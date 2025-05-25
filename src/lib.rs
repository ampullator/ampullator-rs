use std::collections::HashMap;
use std::collections::VecDeque;

pub type Sample = f32;

//------------------------------------------------------------------------------

pub trait GenNode {
    fn process(&mut self, inputs: &[&[Sample]], output: &mut [Sample], sample_rate: f32);
    fn num_inputs(&self) -> usize;
    fn input_names(&self) -> &[&'static str];
}

//------------------------------------------------------------------------------

pub struct SineOscillator {
    phase: Sample,
    default_freq: Sample,
    default_phase_offset: Sample,
    default_min: Sample,
    default_max: Sample,
}

impl SineOscillator {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            default_freq: 440.0,
            default_phase_offset: 0.0,
            default_min: -1.0,
            default_max: 1.0,
        }
    }
}

impl GenNode for SineOscillator {
    fn num_inputs(&self) -> usize {
        4
    } // freq, phase, min, max

    fn input_names(&self) -> &[&'static str] {
        &["freq", "phase", "min", "max"]
    }

    fn process(&mut self, inputs: &[&[Sample]], output: &mut [Sample], sample_rate: f32) {
        let freq_in = inputs.get(0).copied().unwrap_or(&[]);
        let phase_in = inputs.get(1).copied().unwrap_or(&[]);
        let min_in = inputs.get(2).copied().unwrap_or(&[]);
        let max_in = inputs.get(3).copied().unwrap_or(&[]);
        let dt = 1.0 / sample_rate;

        for (i, out) in output.iter_mut().enumerate() {
            let freq = freq_in.get(i).copied().unwrap_or(self.default_freq);
            let phase_offset = phase_in
                .get(i)
                .copied()
                .unwrap_or(self.default_phase_offset);
            let min = min_in.get(i).copied().unwrap_or(self.default_min);
            let max = max_in.get(i).copied().unwrap_or(self.default_max);

            self.phase += freq * dt;
            if self.phase > 1.0 {
                self.phase -= 1.0;
            }

            let norm = ((self.phase + phase_offset) * std::f32::consts::TAU).sin();
            *out = min + (norm + 1.0) * 0.5 * (max - min);
        }
    }
}

//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

pub struct NodeEdge {
    pub input_index: usize, // e.g., 0 for frequency, 1 for phase
    pub source: NodeId,
}

pub struct GraphNode {
    pub id: NodeId,
    pub node: Box<dyn GenNode>,
    pub inputs: Vec<NodeEdge>,
    pub buffer: Vec<Sample>,
}

//------------------------------------------------------------------------------

//------------------------------------------------------------------------------
pub struct GenGraph {
    pub nodes: Vec<GraphNode>,
    pub node_names: HashMap<String, NodeId>,
    pub sample_rate: f32,
    pub buffer_size: usize,
}

impl GenGraph {
    pub fn new(sample_rate: f32, buffer_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            node_names: HashMap::new(),
            sample_rate,
            buffer_size,
        }
    }

    pub fn add_node<N: Into<String>>(
        &mut self,
        name: N,
        node: Box<dyn GenNode>,
    ) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.node_names.insert(name.into(), id);
        self.nodes.push(GraphNode {
            id,
            node,
            inputs: Vec::new(),
            buffer: vec![0.0; self.buffer_size],
        });
        id
    }

    pub fn connect(&mut self, target: NodeId, input_index: usize, source: NodeId) {
        if let Some(target_node) = self.nodes.get_mut(target.0) {
            target_node.inputs.push(NodeEdge {
                input_index,
                source,
            });
        }
    }

    pub fn connect_named(
        &mut self,
        target_name: &str,
        input_name: &str,
        source_name: &str,
    ) {
        let target_id = self.node_names[target_name];
        let source_id = self.node_names[source_name];
        let target_node = &self.nodes[target_id.0];

        let index = target_node
            .node
            .input_names()
            .iter()
            .position(|&name| name == input_name)
            .expect("Invalid input name");

        self.connect(target_id, index, source_id);
    }

    // dependency-respecting order (DAG topological sort):
    pub fn build_execution_order(&self) -> Vec<NodeId> {
        let mut indegree = vec![0; self.nodes.len()];
        for node in &self.nodes {
            let target_idx = node.id.0;
            indegree[target_idx] = node.inputs.len();
        }

        let mut queue: VecDeque<NodeId> = indegree
            .iter()
            .enumerate()
            .filter(|&(_, d)| *d == 0)
            .map(|(i, _)| NodeId(i))
            .collect();

        let mut order = Vec::new();

        while let Some(nid) = queue.pop_front() {
            order.push(nid);
            for target in self.nodes.iter() {
                for edge in &target.inputs {
                    if edge.source == nid {
                        indegree[target.id.0] -= 1;
                        if indegree[target.id.0] == 0 {
                            queue.push_back(target.id);
                        }
                    }
                }
            }
        }

        order
    }

    pub fn process(&mut self) {
        let execution_order = self.build_execution_order();
        for &nid in &execution_order {
            // let len = self.nodes.len();
            let node_index = nid.0;

            // Safely borrow `self.nodes[node_index]` mutably and the rest immutably
            let (left, right) = self.nodes.split_at_mut(node_index);
            let (node, rest) = right.split_first_mut().expect("valid index");

            let mut input_slices: Vec<&[Sample]> =
                vec![&[]; node.node.input_names().len()];
            for edge in &node.inputs {
                input_slices[edge.input_index] = &if edge.source.0 < node_index {
                    left[edge.source.0].buffer.as_slice()
                } else {
                    rest[edge.source.0 - node_index - 1].buffer.as_slice()
                };
            }

            let output = &mut node.buffer;
            node.node.process(&input_slices, output, self.sample_rate);
        }
    }

    pub fn get_output(&self, id: NodeId) -> &[Sample] {
        &self.nodes[id.0].buffer
    }
}

pub fn greet(name: &str) {
    println!("Hello, {}!", name);
}
