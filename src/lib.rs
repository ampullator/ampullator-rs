use std::collections::HashMap;
use std::collections::VecDeque;
use std::str::FromStr;

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
    fn type_name(&self) -> &'static str;
    fn input_names(&self) -> &[&'static str];
    fn output_names(&self) -> &[&'static str];
    fn default_input(&self, _input_name: &str) -> Option<Sample> {
        None
    }
    fn describe_config(&self) -> Option<String> {
        None
    }
}

//------------------------------------------------------------------------------

pub struct ConstantNode {
    value: Sample,
}

impl ConstantNode {
    pub fn new(value: Sample) -> Self {
        Self { value }
    }
}

impl GenNode for ConstantNode {
    fn type_name(&self) -> &'static str {
        "Constant"
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("value = {:.3}", self.value))
    }

    fn input_names(&self) -> &[&'static str] {
        &[]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn process(
        &mut self,
        _inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let out = &mut outputs[0];
        for v in out.iter_mut() {
            *v = self.value;
        }
    }
}


//------------------------------------------------------------------------------


#[derive(Clone, Copy, Debug)]
pub enum FreqUnit {
    Hz,
    Seconds,
    Samples,
    Midi,
    Bpm,
}

impl FromStr for FreqUnit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hz" => Ok(FreqUnit::Hz),
            "sec" | "seconds" => Ok(FreqUnit::Seconds),
            "samples" | "spc" => Ok(FreqUnit::Samples),
            "midi" => Ok(FreqUnit::Midi),
            "bpm" => Ok(FreqUnit::Bpm),
            _ => Err(format!("Unknown frequency unit: {}", s)),
        }
    }
}

pub struct FreqConverterNode {
    mode: FreqUnit,
}

impl FreqConverterNode {
    pub fn new(mode: FreqUnit) -> Self {
        Self { mode }
    }
}

impl GenNode for FreqConverterNode {
    fn type_name(&self) -> &'static str {
        "FreqConverter"
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("mode = {:?}", self.mode).to_lowercase())
    }

    fn input_names(&self) -> &[&'static str] {
        &["in"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["hz"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs.get(0).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let x = input.get(i).copied().unwrap_or(0.0);
            out[i] = match self.mode {
                FreqUnit::Hz => x,
                FreqUnit::Seconds => if x != 0.0 { 1.0 / x } else { 0.0 },
                FreqUnit::Samples => if x != 0.0 { sample_rate / x } else { 0.0 },
                FreqUnit::Midi => 440.0 * 2f32.powf((x - 69.0) / 12.0),
                FreqUnit::Bpm => x / 60.0,
            };
        }
    }
}


//------------------------------------------------------------------------------

pub struct SumNode {
    input_labels: Vec<String>,
    input_refs: Vec<&'static str>,
}

impl SumNode {
    pub fn new(input_count: usize) -> Self {
        let input_labels: Vec<String> =
            (0..input_count).map(|i| format!("in{}", i)).collect();

        // Promote to 'static using Box::leak safely
        let input_refs: Vec<&'static str> = input_labels
            .iter()
            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
            .collect();

        Self {
            input_labels,
            input_refs,
        }
    }
}

impl GenNode for SumNode {
    fn type_name(&self) -> &'static str {
        "Sum"
    }

    fn input_names(&self) -> &[&'static str] {
        &self.input_refs
    }

    fn output_names(&self) -> &[&'static str] {
        &["sum"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let out = &mut outputs[0];
        let len = out.len();

        match inputs.len() {
            2 => {
                let a = inputs[0];
                let b = inputs[1];
                for i in 0..len {
                    out[i] = a[i] + b[i];
                }
            }
            _ => {
                for i in 0..len {
                    let mut acc = 0.0;
                    for input in inputs {
                        acc += input[i];
                    }
                    out[i] = acc;
                }
            }
        }
    }
}

//------------------------------------------------------------------------------

pub struct OscSine {
    phase: Sample,
    default_freq: Sample,
    default_phase_offset: Sample,
    default_min: Sample,
    default_max: Sample,
}

impl OscSine {
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

impl GenNode for OscSine {
    fn type_name(&self) -> &'static str {
        "OscSine"
    }

    fn input_names(&self) -> &[&'static str] {
        &["freq", "phase", "min", "max"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["wave", "trigger"]
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "freq" => Some(self.default_freq),
            "phase" => Some(self.default_phase_offset),
            "min" => Some(self.default_min),
            "max" => Some(self.default_max),
            _ => None,
        }
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
            let phase_offset = phase_in
                .get(i)
                .copied()
                .unwrap_or(self.default_phase_offset);
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
            //     println!("OscSine activated at t=0s");
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
    pub fn add_node<N: Into<String>>(
        &mut self,
        name: N,
        node: Box<dyn GenNode>,
    ) -> NodeId {
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

    pub fn connect(
        &mut self,
        target: NodeId,
        input_index: usize,
        source: NodeId,
        output_index: usize,
    ) {
        if let Some(target_node) = self.nodes.get_mut(target.0) {
            if target_node
                .inputs
                .iter()
                .any(|e| e.input_index == input_index)
            {
                panic!(
                    "Input {} on node {:?} is already connected. \
                     Only one connection per input is allowed.",
                    input_index, target
                );
            }
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

            let mut output_slices: Vec<&mut [Sample]> = node
                .outputs
                .iter_mut()
                .map(|buf| buf.as_mut_slice())
                .collect();

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

    // pub fn describe(&self) -> String {
    //     let mut lines = Vec::new();

    //     for node in &self.nodes {
    //         let name = self
    //             .node_names
    //             .iter()
    //             .find(|&(_, id)| *id == node.id)
    //             .map(|(name, _)| name.as_str())
    //             .unwrap_or("(unnamed)");

    //         lines.push(format!("[{}] {}", node.node.type_name(), name));

    //         // Input section
    //         if !node.node.input_names().is_empty() {
    //             lines.push("  input:".to_string());

    //             for (i, input_name) in node.node.input_names().iter().enumerate() {
    //                 let source = node.inputs.iter().find(|e| e.input_index == i);
    //                 if let Some(edge) = source {
    //                     let source_name = self
    //                         .node_names
    //                         .iter()
    //                         .find(|&(_, id)| *id == edge.source)
    //                         .map(|(name, _)| name.as_str())
    //                         .unwrap_or("(unknown)");
    //                     let source_node = &self.nodes[edge.source.0];
    //                     let output_name = source_node
    //                         .node
    //                         .output_names()
    //                         .get(edge.output_index)
    //                         .copied()
    //                         .unwrap_or("???");
    //                     lines.push(format!("    {} <- {}.{}", input_name, source_name, output_name));
    //                 } else {
    //                     lines.push(format!("    {} <- (unconnected)", input_name));
    //                 }
    //             }
    //         }

    //         // Output section
    //         let output_names = node.node.output_names();
    //         if !output_names.is_empty() {
    //             lines.push(format!(
    //                 "  output: {}",
    //                 output_names.join(", ")
    //             ));
    //         }

    //         lines.push("".to_string());
    //     }

    //     lines.join("\n")
    // }

    pub fn describe(&self) -> String {
        let mut lines = Vec::new();

        for &node_id in &self.build_execution_order() {
            let node = &self.nodes[node_id.0];

            let name = self
                .node_names
                .iter()
                .find(|&(_, &id)| id == node_id)
                .map(|(n, _)| n.as_str())
                .unwrap_or("(unnamed)");

            let type_name = node.node.type_name();
            let config = node.node.describe_config();
            match config {
                Some(cfg) => lines.push(format!("[{}] {} {{ {} }}", type_name, name, cfg)),
                None => lines.push(format!("[{}] {}", type_name, name)),
            }

            // Inputs
            for (i, input_name) in node.node.input_names().iter().enumerate() {
                let source = node.inputs.iter().find(|e| e.input_index == i);
                if let Some(edge) = source {
                    let source_name = self
                        .node_names
                        .iter()
                        .find(|&(_, id)| *id == edge.source)
                        .map(|(n, _)| n.as_str())
                        .unwrap_or("(unknown)");
                    let output_name = self.nodes[edge.source.0]
                        .node
                        .output_names()
                        .get(edge.output_index)
                        .copied()
                        .unwrap_or("???");
                    lines.push(format!("    {:<8} ← {}.{}", input_name, source_name, output_name));
                } else if let Some(default) = node.node.default_input(input_name) {
                    lines.push(format!("    {:<8} ← [default: {:.3}]", input_name, default));
                } else {
                    lines.push(format!("    {:<8} ← (unconnected)", input_name));
                }
            }

            // Outputs with current value
            for (i, output_name) in node.node.output_names().iter().enumerate() {
                let value = node.outputs.get(i)
                    .and_then(|buf| buf.last())
                    .map(|v| format!("{:.3}", v))
                    .unwrap_or_else(|| "(empty)".to_string());
                lines.push(format!("    → {:<8} = {}", output_name, value));
            }

            lines.push("".to_string());
        }

        lines.join("\n")
    }

}
