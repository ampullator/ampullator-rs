use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::Write;
use std::process::Command;
use std::str::FromStr;
use tempfile::NamedTempFile;
use std::path::Path;

pub type Sample = f32;

fn split_name(s: &str) -> (&str, &str) {
    s.rsplit_once('.')
        .unwrap_or_else(|| panic!("Expected 'name.port', got: '{}'", s))
}

//------------------------------------------------------------------------------
// Alt names: UnitGen

pub trait UGen {
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

pub struct UGConst {
    value: Sample,
}

impl UGConst {
    pub fn new(value: Sample) -> Self {
        Self { value }
    }
}

impl UGen for UGConst {
    fn type_name(&self) -> &'static str {
        "UGConst"
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
pub enum UnitRate {
    Hz,
    Seconds,
    Samples,
    Midi,
    Bpm,
}

impl FromStr for UnitRate {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hz" => Ok(UnitRate::Hz),
            "sec" | "seconds" => Ok(UnitRate::Seconds),
            "samples" | "spc" => Ok(UnitRate::Samples),
            "midi" => Ok(UnitRate::Midi),
            "bpm" => Ok(UnitRate::Bpm),
            _ => Err(format!("Unknown frequency unit: {}", s)),
        }
    }
}

pub struct UGAsHz {
    mode: UnitRate,
}

impl UGAsHz {
    pub fn new(mode: UnitRate) -> Self {
        Self { mode }
    }
}

impl UGen for UGAsHz {
    fn type_name(&self) -> &'static str {
        "UGAsHz"
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
                UnitRate::Hz => x,
                UnitRate::Seconds => {
                    if x != 0.0 {
                        1.0 / x
                    } else {
                        0.0
                    }
                }
                UnitRate::Samples => {
                    if x != 0.0 {
                        sample_rate / x
                    } else {
                        0.0
                    }
                }
                UnitRate::Midi => 440.0 * 2f32.powf((x - 69.0) / 12.0),
                UnitRate::Bpm => x / 60.0,
            };
        }
    }
}

//------------------------------------------------------------------------------
#[derive(Debug)]
pub enum ModeRound {
    Round,
    Floor,
    Ceil,
}

#[derive(Debug)]
pub struct UGRound {
    places: i32,
    factor: f32,
    mode: ModeRound,
}

impl UGRound {
    pub fn new(places: i32, mode: ModeRound) -> Self {
        let factor = 10f32.powi(places);
        Self {
            places,
            factor,
            mode,
        }
    }
}

impl UGen for UGRound {
    fn type_name(&self) -> &'static str {
        "UGRound"
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("places = {}, mode = {:?}", self.places, self.mode))
    }

    fn input_names(&self) -> &[&'static str] {
        &["in"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs.get(0).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        let factor = self.factor;
        match self.mode {
            ModeRound::Round => {
                for (i, v) in out.iter_mut().enumerate() {
                    let x = input.get(i).copied().unwrap_or(0.0);
                    *v = (x * factor).round() / factor;
                }
            }
            ModeRound::Floor => {
                for (i, v) in out.iter_mut().enumerate() {
                    let x = input.get(i).copied().unwrap_or(0.0);
                    *v = (x * factor).floor() / factor;
                }
            }
            ModeRound::Ceil => {
                for (i, v) in out.iter_mut().enumerate() {
                    let x = input.get(i).copied().unwrap_or(0.0);
                    *v = (x * factor).ceil() / factor;
                }
            }
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGSum {
    input_labels: Vec<String>,
    input_refs: Vec<&'static str>,
}

impl UGSum {
    pub fn new(input_count: usize) -> Self {
        if input_count <= 1 {
            panic!("Input count should be greater than 1");
        }
        // input labels wil start with in1, ..., inN
        let input_labels: Vec<String> =
            (1..input_count + 1).map(|i| format!("in{}", i)).collect();
        println!("{:?}", input_labels);
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

impl UGen for UGSum {
    fn type_name(&self) -> &'static str {
        "UGSum"
    }

    fn input_names(&self) -> &[&'static str] {
        &self.input_refs
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
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

pub struct UGWhite {
    default_min: Sample,
    default_max: Sample,
    rng: StdRng,
    seed: Option<u64>,
}

impl UGWhite {
    /// Create a new white noise generator. If `seed` is `None`, a random seed is used.
    pub fn new(seed: Option<u64>) -> Self {
        let actual_seed = seed.unwrap_or_else(|| rand::rng().random());
        Self {
            default_min: -1.0,
            default_max: 1.0,
            rng: StdRng::seed_from_u64(actual_seed),
            seed: seed, // original user-provided seed
        }
    }
}

impl UGen for UGWhite {
    fn type_name(&self) -> &'static str {
        "UGWhite"
    }

    fn input_names(&self) -> &[&'static str] {
        &["min", "max"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "min" => Some(self.default_min),
            "max" => Some(self.default_max),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        self.seed.map(|s| format!("seed = {}", s))
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let min_in = inputs.get(0).copied().unwrap_or(&[]);
        let max_in = inputs.get(1).copied().unwrap_or(&[]);

        let out = &mut outputs[0];
        for (i, v) in out.iter_mut().enumerate() {
            let min = min_in.get(i).copied().unwrap_or(self.default_min);
            let max = max_in.get(i).copied().unwrap_or(self.default_max);
            *v = self.rng.random_range(min..=max);
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGSine {
    phase: Sample,
    default_freq: Sample,
    default_phase_offset: Sample,
    default_min: Sample,
    default_max: Sample,
}

impl UGSine {
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

impl UGen for UGSine {
    fn type_name(&self) -> &'static str {
        "UGSine"
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
        }
    }
}
//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

pub struct NodeEdge {
    pub src: NodeId,
    pub output_index: usize, // output in src
    pub input_index: usize,  // index in the Node's inputs
}

// `inputs` are not sorted; each NodeEdge defines src and an output of that src to be delivered to this nodes's input.
// `outputs` are sorted in fixed output positions.
pub struct GraphNode {
    pub id: NodeId,
    pub node: Box<dyn UGen>,
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
    pub fn add_node<N: Into<String>>(&mut self, name: N, node: Box<dyn UGen>) -> NodeId {
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

    fn connect_ids(
        &mut self,
        src: NodeId,
        output_index: usize,
        dst: NodeId,
        input_index: usize,
    ) {
        if let Some(dst_node) = self.nodes.get_mut(dst.0) {
            if dst_node.inputs.iter().any(|e| e.input_index == input_index) {
                panic!(
                    "Input {} on node {:?} is already connected. \
                     Only one connection per input is allowed.",
                    input_index, dst
                );
            }
            // connect the src.out to dst.in
            dst_node.inputs.push(NodeEdge {
                src,
                output_index,
                input_index,
            });
        }
    }

    pub fn connect(&mut self, src: &str, dst: &str) {
        let (src_name, output_name) = split_name(src);
        let (dst_name, input_name) = split_name(dst);

        let dst_id = self.node_names[dst_name];
        let src_id = self.node_names[src_name];
        let dst_node = &self.nodes[dst_id.0];
        let src_node = &self.nodes[src_id.0];

        let input_index = dst_node
            .node
            .input_names()
            .iter()
            .position(|&name| name == input_name)
            .expect(format!("For {dst_name}, invalid input name: {input_name}").as_str());

        let output_index = src_node
            .node
            .output_names()
            .iter()
            .position(|&name| name == output_name)
            .expect(
                format!("For {src_name}, invalid output name: {output_name}").as_str(),
            );

        self.connect_ids(src_id, output_index, dst_id, input_index);
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
                    if edge.src == nid {
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
                input_slices[edge.input_index] = if edge.src.0 < node_index {
                    &left[edge.src.0].outputs[edge.output_index]
                } else {
                    &rest[edge.src.0 - node_index - 1].outputs[edge.output_index]
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

    pub fn get_output_named(&self, name: &str) -> &[Sample] {
        let (node_name, output_name) = split_name(name);
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

    //--------------------------------------------------------------------------
    pub fn describe_json(&self) -> Value {
        let mut result = Vec::new();

        for &node_id in &self.build_execution_order() {
            let node = &self.nodes[node_id.0];

            let name = self
                .node_names
                .iter()
                .find(|&(_, &id)| id == node_id)
                .map(|(n, _)| n.clone())
                .unwrap_or_else(|| format!("node_{}", node_id.0));

            let type_name = node.node.type_name();
            let config_str = node.node.describe_config();

            let inputs: Vec<Value> = node
                .node
                .input_names()
                .iter()
                .enumerate()
                .map(|(i, input_name)| {
                    let src = node.inputs.iter().find(|e| e.input_index == i);
                    match src {
                        Some(edge) => {
                            let src_name = self
                                .node_names
                                .iter()
                                .find(|&(_, id)| *id == edge.src)
                                .map(|(n, _)| n.clone())
                                .unwrap_or_else(|| format!("node_{}", edge.src.0));

                            let output_name = self.nodes[edge.src.0]
                                .node
                                .output_names()
                                .get(edge.output_index)
                                .copied()
                                .unwrap_or("???");

                            json!({
                                "name": input_name,
                                "connected_to": {
                                    "node": src_name,
                                    "output": output_name
                                }
                            })
                        }
                        None => {
                            let default = node.node.default_input(input_name);
                            json!({
                                "name": input_name,
                                "default": default
                            })
                        }
                    }
                })
                .collect();

            let outputs: Vec<Value> = node
                .node
                .output_names()
                .iter()
                .enumerate()
                .map(|(i, output_name)| {
                    let value = node.outputs.get(i).and_then(|buf| buf.last()).copied();
                    json!({
                        "name": output_name,
                        "value": value
                    })
                })
                .collect();

            result.push(json!({
                "id": node.id.0,
                "name": name,
                "type": type_name,
                "config": config_str,
                "inputs": inputs,
                "outputs": outputs
            }));
        }

        Value::Array(result)
    }

    pub fn describe(&self) -> String {
        let data = self.describe_json();
        let mut lines = Vec::new();

        for node in data.as_array().expect("JSON array of nodes") {
            let type_name = node["type"].as_str().unwrap_or("UnknownType");
            let name = node["name"].as_str().unwrap_or("unknown");
            let config = node["config"].as_str().unwrap_or("");

            if config.is_empty() {
                lines.push(format!("[{}] {}", type_name, name));
            } else {
                lines.push(format!("[{}] {} {{ {} }}", type_name, name, config));
            }

            // Inputs
            for input in node["inputs"].as_array().unwrap() {
                let label = input["name"].as_str().unwrap_or("?");
                if let Some(obj) = input.get("connected_to") {
                    let src_node = obj["node"].as_str().unwrap_or("?");
                    let src_output = obj["output"].as_str().unwrap_or("?");
                    lines.push(format!("    {} ← {}.{}", label, src_node, src_output));
                } else if let Some(val) = input.get("default").and_then(|v| v.as_f64()) {
                    lines.push(format!("    {} ←= {:.3}", label, val));
                } else {
                    lines.push(format!("    {} ← ∅", label));
                }
            }

            // Outputs
            for output in node["outputs"].as_array().unwrap() {
                let label = output["name"].as_str().unwrap_or("?");
                let value = output["value"]
                    .as_f64()
                    .map(|v| format!("{:.3}", v))
                    .unwrap_or_else(|| "(empty)".to_string());
                lines.push(format!("    → {} ≊ {}", label, value));
            }

            lines.push("".to_string());
        }

        lines.join("\n")
    }

    //--------------------------------------------------------------------------
    pub fn to_gnuplot(&self, output_png_path: &Path) -> String {
        let outputs = self
            .build_execution_order()
            .into_iter()
            .flat_map(|nid| {
                let node = &self.nodes[nid.0];
                let name = self
                    .node_names
                    .iter()
                    .find(|&(_, &id)| id == nid)
                    .map(|(n, _)| n.clone())
                    .unwrap_or_else(|| format!("node_{}", nid.0));
                node.node
                    .output_names()
                    .iter()
                    .enumerate()
                    .map(move |(i, output_name)| {
                        let values = &node.outputs[i];
                        (format!("{}.{}", name, output_name), values)
                    })
            })
            .collect::<Vec<_>>();

        let d = outputs.len();
        // let margin = 0.04;
        // let height = 1.0 / d as f64;

        let mut script = String::new();

        script.push_str("set terminal pngcairo size 800,600\n");
        script.push_str(&format!(
            "set output '{}'\n\n",
            output_png_path.display()
        ));
        script.push_str(
            r#"# General appearance
set style line 11 lc rgb '#ffffff' lt 1
set tics out nomirror scale 0,0.001
set format y "%g"
unset key
set grid
set lmargin screen 0.15
set rmargin screen 0.98
set ytics font ",8"
unset xtics

# Color and style setup
do for [i=1:3] {
    set style line i lt 1 lw 1 pt 3 lc rgb '#332255'
}

# Multiplot setup
set multiplot
"#
        );

        script.push_str(&format!("d = {}\n", d));
        script.push_str("margin = 0.04\n");
        script.push_str("height = 1.0 / d\n");
        script.push_str("pos = 1.0\n\n");

        script.push_str("label_x = 0.06\n");
        script.push_str("label_font = \",9\"\n\n");

        for (i, (label, values)) in outputs.iter().enumerate() {
            let panel = i + 1;
            script.push_str(&format!(
                r#"# Panel {}
top = pos - margin * {}
bottom = pos - height + margin * 0.5
pos = pos - height
set tmargin screen top
set bmargin screen bottom
set label {} "{}" at screen label_x, screen (bottom + height / 2) center font label_font
plot '-' using 1 with linespoints linestyle {}
"#,
                panel,
                if i == 0 { 1.0 } else { 0.5 },
                panel,
                label,
                (i % 3) + 1
            ));

            for v in *values {
                script.push_str(&format!("{}\n", v));
            }
            script.push_str("e\n\n");
        }

        script.push_str("unset multiplot\n");
        for i in 1..=d {
            script.push_str(&format!("unset label {}\n", i));
        }

        script
    }


}

pub fn plot_graph_to_image(graph: &GenGraph, image_path: &str) -> std::io::Result<()> {
    let script = graph.to_gnuplot(image_path.as_ref());
    let mut file = NamedTempFile::new()?;
    write!(file, "{script}")?;
    let script_path = file.path();
    let status = Command::new("gnuplot").arg(script_path).status()?;

    if !status.success() {
        eprintln!("gnuplot failed with exit code: {:?}", status.code());
    }

    Ok(())
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_graph_describe_json_a() {
        let mut graph = GenGraph::new(44100.0, 128);

        graph.add_node("note", Box::new(UGConst::new(69.0))); // A4
        graph.add_node("conv", Box::new(UGAsHz::new(UnitRate::Midi)));
        graph.add_node("osc", Box::new(UGSine::new()));

        graph.connect("note.out", "conv.in");
        graph.connect("conv.hz", "osc.freq");

        assert_eq!(
            graph.describe_json().to_string(),
            r#"[{"config":"value = 69.000","id":0,"inputs":[],"name":"note","outputs":[{"name":"out","value":0.0}],"type":"UGConst"},{"config":"mode = midi","id":1,"inputs":[{"connected_to":{"node":"note","output":"out"},"name":"in"}],"name":"conv","outputs":[{"name":"hz","value":0.0}],"type":"UGAsHz"},{"config":null,"id":2,"inputs":[{"connected_to":{"node":"conv","output":"hz"},"name":"freq"},{"default":0.0,"name":"phase"},{"default":-1.0,"name":"min"},{"default":1.0,"name":"max"}],"name":"osc","outputs":[{"name":"wave","value":0.0},{"name":"trigger","value":0.0}],"type":"UGSine"}]"#
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_constant_a() {
        let c1 = UGConst::new(3.0);
        assert_eq!(c1.type_name(), "UGConst");

        let mut g = GenGraph::new(120.0, 8);
        g.add_node("c1", Box::new(c1));
        g.process();
        assert_eq!(
            g.get_output_named("c1.out"),
            vec![3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_sum_a() {
        let c1 = UGConst::new(3.0);
        let c2 = UGConst::new(2.0);
        let s1 = UGSum::new(2); // input count

        let mut g = GenGraph::new(120.0, 8);
        g.add_node("c1", Box::new(c1));
        g.add_node("c2", Box::new(c2));
        g.add_node("s1", Box::new(s1));
        g.connect("c1.out", "s1.in1");
        g.connect("c2.out", "s1.in2");
        g.process();

        assert_eq!(
            g.get_output_named("s1.out"),
            vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_sine_a() {
        let c1 = UGConst::new(1.0);
        let osc1 = UGSine::new();
        let r1 = UGRound::new(1, ModeRound::Round);

        let mut g = GenGraph::new(8.0, 8);
        g.add_node("c1", Box::new(c1));
        g.add_node("osc1", Box::new(osc1));
        g.add_node("r1", Box::new(r1));

        g.connect("c1.out", "osc1.freq");
        g.connect("osc1.wave", "r1.in");

        g.process();

        assert_eq!(
            g.get_output_named("r1.out"),
            vec![0.7, 1.0, 0.7, -0.0, -0.7, -1.0, -0.7, 0.0]
        );

        plot_graph_to_image(&g, "/tmp/ampullator.png").unwrap();
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_white_a() {
        let n1 = UGWhite::new(Some(42));
        let r1 = UGRound::new(2, ModeRound::Round);

        let mut g = GenGraph::new(8.0, 8);
        g.add_node("n1", Box::new(n1));
        g.add_node("r1", Box::new(r1));
        g.connect("n1.out", "r1.in");

        g.process();

        assert_eq!(
            g.get_output_named("r1.out"),
            vec![-0.73, 0.05, -0.5, 0.09, 0.74, 0.27, 0.98, -0.19]
        )
    }
}
