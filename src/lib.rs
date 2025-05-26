use std::collections::HashMap;
use std::collections::VecDeque;

pub type Sample = f32;

//------------------------------------------------------------------------------


pub trait GenNode {
    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        time_sample: usize,
    );

    fn input_names(&self) -> &[&'static str];
    fn output_names(&self) -> &[&'static str];
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
    fn input_names(&self) -> &[&'static str] {
        &["freq", "phase", "min", "max"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["wave", "trigger"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let freq_in = inputs.get(0).copied().unwrap_or(&[]);
        let phase_in = inputs.get(1).copied().unwrap_or(&[]);
        let min_in = inputs.get(2).copied().unwrap_or(&[]);
        let max_in = inputs.get(3).copied().unwrap_or(&[]);

        let (wave_out, rest) = outputs.split_at_mut(1);
        let wave_out = &mut wave_out[0];
        let trig_out = &mut rest[0];

        let dt = 1.0 / sample_rate;

        for i in 0..wave_out.len() {
            // let global_sample = time_sample + i;

            let freq = freq_in.get(i).copied().unwrap_or(self.default_freq);
            let phase_offset = phase_in.get(i).copied().unwrap_or(self.default_phase_offset);
            let min = min_in.get(i).copied().unwrap_or(self.default_min);
            let max = max_in.get(i).copied().unwrap_or(self.default_max);

            self.phase += freq * dt;
            let crossed = self.phase >= 1.0;
            if crossed {
                self.phase -= 1.0;
            }

            let norm = ((self.phase + phase_offset) * std::f32::consts::TAU).sin();
            wave_out[i] = min + (norm + 1.0) * 0.5 * (max - min);
            trig_out[i] = if crossed { 1.0 } else { 0.0 };

            // if global_sample == 0 {
            //     println!("SineOscillator activated at t=0s");
            // }
        }
    }
}
//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

pub struct NodeEdge {
    pub input_index: usize,
    pub source: NodeId,
    pub output_index: usize,
}
pub struct GraphNode {
    pub id: NodeId,
    pub node: Box<dyn GenNode>,
    pub inputs: Vec<NodeEdge>,
    pub outputs: Vec<Vec<Sample>>,
}
//------------------------------------------------------------------------------
pub struct GenGraph {
    pub nodes: Vec<GraphNode>,
    pub node_names: HashMap<String, NodeId>,
    pub sample_rate: f32,
    pub buffer_size: usize,
    pub time_sample: usize,
}

impl GenGraph {
    pub fn new(sample_rate: f32, buffer_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            node_names: HashMap::new(),
            sample_rate,
            buffer_size,
            time_sample: 0,
        }
    }
    pub fn add_node<N: Into<String>>(&mut self, name: N, node: Box<dyn GenNode>) -> NodeId {
        let id = NodeId(self.nodes.len());
        let output_count = node.output_names().len();
        self.node_names.insert(name.into(), id);
        self.nodes.push(GraphNode {
            id,
            node,
            inputs: Vec::new(),
            outputs: vec![vec![0.0; self.buffer_size]; output_count],
        });
        id
    }

    pub fn connect(&mut self, target: NodeId, input_index: usize, source: NodeId, output_index: usize) {
        if let Some(target_node) = self.nodes.get_mut(target.0) {
            target_node.inputs.push(NodeEdge {
                input_index,
                source,
                output_index,
            });
        }
    }
    pub fn connect_named(
        &mut self,
        target_name: &str,
        input_name: &str,
        source_name: &str,
        output_name: &str,
    ) {
        let target_id = self.node_names[target_name];
        let source_id = self.node_names[source_name];
        let target_node = &self.nodes[target_id.0];
        let source_node = &self.nodes[source_id.0];

        let input_index = target_node
            .node
            .input_names()
            .iter()
            .position(|&name| name == input_name)
            .expect("Invalid input name");

        let output_index = source_node
            .node
            .output_names()
            .iter()
            .position(|&name| name == output_name)
            .expect("Invalid output name");

        self.connect(target_id, input_index, source_id, output_index);
    }

    // dependency-respecting order (DAG topological sort):
    pub fn build_execution_order(&self) -> Vec<NodeId> {
        let mut indegree = vec![0; self.nodes.len()];
        for node in &self.nodes {
            indegree[node.id.0] = node.inputs.len();
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
            for target in &self.nodes {
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
            let node_index = nid.0;
            let (left, right) = self.nodes.split_at_mut(node_index);
            let (node, rest) = right.split_first_mut().expect("valid index");

            let mut input_slices: Vec<&[Sample]> =
                vec![&[]; node.node.input_names().len()];
            for edge in &node.inputs {
                input_slices[edge.input_index] = if edge.source.0 < node_index {
                    &left[edge.source.0].outputs[edge.output_index]
                } else {
                    &rest[edge.source.0 - node_index - 1].outputs[edge.output_index]
                };
            }

            let mut output_slices: Vec<&mut [Sample]> =
                node.outputs.iter_mut().map(|buf| buf.as_mut_slice()).collect();

            node.node.process(
                &input_slices,
                &mut output_slices,
                self.sample_rate,
                self.time_sample,
            );
        }

        self.time_sample += self.buffer_size;
    }

    pub fn get_output(&self, node_name: &str, output_name: &str) -> &[Sample] {
        let node_id = self.node_names[node_name];
        let node = &self.nodes[node_id.0];
        let index = node
            .node
            .output_names()
            .iter()
            .position(|&name| name == output_name)
            .expect("Invalid output name");
        &node.outputs[index]
    }

}

// pub fn greet(name: &str) {
//     println!("Hello, {}!", name);
// }
