use serde_json::{Value, json};
use std::collections::HashMap;
use std::collections::VecDeque;

use crate::ugen_core::UGen;
use crate::util::Sample;
use crate::util::split_name;

//------------------------------------------------------------------------------
//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

pub struct NodeEdge {
    pub(crate) src: NodeId,
    pub(crate) output_index: usize, // output in src
    pub(crate) input_index: usize,  // index in the Node's inputs
}

pub struct GraphNode {
    pub(crate) id: NodeId,
    pub(crate) name: String,
    pub(crate) node: Box<dyn UGen>,
    // `inputs` are unsorted `NodeEdge``, defining a node, an output of that node to read from, the input to apply to this node.
    pub(crate) inputs: Vec<NodeEdge>,
    // Output samples from `process()` are stored in the `GraphNode`
    pub(crate) outputs: Vec<Vec<Sample>>,
    pub(crate) name_to_output_index: HashMap<String, usize>,
}
//------------------------------------------------------------------------------
pub struct GenGraph {
    // Store nodes as assigned, were pos is NodId
    pub nodes: Vec<GraphNode>,
    // Reverse mapping from node name to NodeId
    pub name_to_node_id: HashMap<String, NodeId>,
    pub sample_rate: f32,
    pub buffer_size: usize,
    pub time_sample: usize,
}

impl GenGraph {
    /// Create a new graph. Creates an empty Vec and HashMap for storing `nodes`` and `name_to_node_id``.
    pub fn new(sample_rate: f32, buffer_size: usize) -> Self {
        Self {
            nodes: Vec::new(),
            name_to_node_id: HashMap::new(),
            sample_rate,
            buffer_size,
            time_sample: 0,
        }
    }

    /// Add a node given a string-convertible name and a `Box` `Ugen`.
    pub fn add_node<N: Into<String>>(
        &mut self,
        name_raw: N,
        node: Box<dyn UGen>,
    ) -> NodeId {
        // `id` is always the position in the `nodes` `Vec`
        let id = NodeId(self.nodes.len());
        let output_count = node.output_names().len();
        let name: String = name_raw.into();

        let mut name_to_output_index = HashMap::new();
        for (i, out_name) in node.output_names().iter().enumerate() {
            name_to_output_index.insert(out_name.to_string(), i);
        }

        if self.name_to_node_id.contains_key(&name) {
            panic!(
                "Node name {} already exists.", name
            );
        }
        self.name_to_node_id.insert(name.clone(), id);
        self.nodes.push(GraphNode {
            id,
            name,
            node,
            inputs: Vec::new(),
            outputs: vec![vec![0.0; self.buffer_size]; output_count],
            name_to_output_index,
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

        let dst_id = self.name_to_node_id[dst_name];
        let src_id = self.name_to_node_id[src_name];
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
    // TODO: this needs to be cached!
    pub fn get_execution_node_ids(&self) -> Vec<NodeId> {
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

    pub fn get_execution_names(&self) -> Vec<String> {
        let mut post = Vec::new();
        for &node_id in &self.get_execution_node_ids() {
            post.push(self.nodes[node_id.0].name.clone());
        }
        post
    }

    pub fn process(&mut self) {
        for &nid in &self.get_execution_node_ids() {
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

    // Given a two-part name node.output, return a slice of the samples for that node and output,
    pub fn get_output_named(&self, name: &str) -> &[Sample] {
        let (node_name, output_name) = split_name(name);
        let node_id = self.name_to_node_id[node_name];
        let node = &self.nodes[node_id.0];
        // TODO: this linear search for outputs should be improved
        let index = node
            .node
            .output_names()
            .iter() // iter over all outputs for this node and find target match
            .position(|&name| name == output_name)
            .expect("Invalid output name");
        &node.outputs[index]
    }

    // NOTE: this is a bit heavy as we create a Vec for each call
    pub fn get_outputs(&self) -> Vec<(String, &Vec<Sample>)> {
        let mut result = Vec::new();

        for &node_id in &self.get_execution_node_ids() {
            let node = &self.nodes[node_id.0];
            let name = &node.name;
            let output_names = node.node.output_names();

            for (i, output_name) in output_names.iter().enumerate() {
                let label = format!("{}.{}", name, output_name);
                result.push((label, &node.outputs[i]));
            }
        }
        result
    }

    // Return all node.output labels
    pub fn get_output_names(&self) -> Vec<String> {
        let mut result = Vec::new();

        for &node_id in &self.get_execution_node_ids() {
            let node = &self.nodes[node_id.0];
            let name = &node.name;
            let output_names = node.node.output_names();
            for output_name in output_names.iter() {
                let label = format!("{}.{}", name, output_name);
                result.push(label);
            }
        }
        result
    }

    //--------------------------------------------------------------------------
    pub fn describe_json(&self) -> Value {
        let mut result = Vec::new();

        for &node_id in &self.get_execution_node_ids() {
            let node = &self.nodes[node_id.0];

            let name = self
                .name_to_node_id
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
                                .name_to_node_id
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
                lines.push(format!("{} <{} {{}}>", name, type_name));
            } else {
                lines.push(format!("{} <{} {{{}}}>", name, type_name, config));
            }

            // Inputs
            for input in node["inputs"].as_array().unwrap() {
                let label = input["name"].as_str().unwrap_or("?");
                if let Some(obj) = input.get("connected_to") {
                    let src_node = obj["node"].as_str().unwrap_or("?");
                    let src_output = obj["output"].as_str().unwrap_or("?");
                    lines.push(format!("{} ← {}.{}", label, src_node, src_output));
                } else if let Some(val) = input.get("default").and_then(|v| v.as_f64()) {
                    lines.push(format!("{} ←= {:.3}", label, val));
                } else {
                    lines.push(format!("{} ← ∅", label));
                }
            }

            // Outputs
            for output in node["outputs"].as_array().unwrap() {
                let label = output["name"].as_str().unwrap_or("?");
                let value = output["value"]
                    .as_f64()
                    .map(|v| format!("{:.3}", v))
                    .unwrap_or_else(|| "(empty)".to_string());
                lines.push(format!("→ {} ≊ {}", label, value));
            }

            lines.push("".to_string());
        }

        lines.join("\n")
    }

    //--------------------------------------------------------------------------
    //     pub fn to_gnuplot(&self, output: &Path) -> String {
    //         let outputs = self
    //             .get_execution_node_ids()
    //             .into_iter()
    //             .flat_map(|nid| {
    //                 let node = &self.nodes[nid.0];
    //                 let name = self
    //                     .name_to_node_id
    //                     .iter()
    //                     .find(|&(_, &id)| id == nid)
    //                     .map(|(n, _)| n.clone())
    //                     .unwrap_or_else(|| format!("node_{}", nid.0));
    //                 node.node.output_names().iter().enumerate().map(
    //                     move |(i, output_name)| {
    //                         let values = &node.outputs[i];
    //                         (format!("{}.{}", name, output_name), values)
    //                     },
    //                 )
    //             })
    //             .collect::<Vec<_>>();

    //         let d = outputs.len();
    //         let mut script = String::new();

    //         script.push_str("set terminal pngcairo size 800,600 background rgb '#12131E'\n");
    //         // script.push_str("set terminal pdfcairo size 8in,6in\n");
    //         script.push_str(&format!("set output '{}'\n\n", output.display()));
    //         script.push_str(
    //             r#"# General appearance
    // set style line 11 lc rgb '#ffffff' lt 1
    // set tics out nomirror scale 0,0.001
    // set format y "%g"
    // unset key
    // set grid
    // set lmargin screen 0.15
    // set rmargin screen 0.98
    // set ytics font ",8"
    // unset xtics

    // # Color and style setup
    // do for [i=1:3] {
    //     set style line i lt 1 lw 1 pt 3 lc rgb '#5599ff'
    // }

    // # Multiplot setup
    // set multiplot
    // "#,
    //         );

    //         script.push_str(&format!("d = {}\n", d));
    //         script.push_str("margin = 0.04\n");
    //         script.push_str("height = 1.0 / d\n");
    //         script.push_str("pos = 1.0\n\n");

    //         script.push_str("label_x = 0.06\n");
    //         script.push_str("label_font = \",9\"\n\n");

    //         for (i, (label, values)) in outputs.iter().enumerate() {
    //             let panel = i + 1;
    //             let block_label = label.replace(['.', '-', ' '], "_");

    //             // Data block
    //             script.push_str(&format!("${} << EOD\n", block_label));
    //             for v in *values {
    //                 script.push_str(&format!("{}\n", v));
    //             }
    //             script.push_str("EOD\n");

    //             // Plot setup
    //             script.push_str(&format!(
    //                 r#"
    //         # Panel {}
    //         top = pos - margin * {}
    //         bottom = pos - height + margin * 0.5
    //         pos = pos - height
    //         set tmargin screen top
    //         set bmargin screen bottom
    //         set label textcolor rgb '#c4c5bf'
    //         set border lc rgb '#c4c5bf'
    //         set grid lc rgb '#cccccc'

    //         set label {} "{}" at screen label_x, screen (bottom + height / 2) center font label_font
    //         plot ${} using 1 with linespoints linestyle {}
    //         "#,
    //                 panel,
    //                 if i == 0 { 1.0 } else { 0.5 },
    //                 panel,
    //                 label,
    //                 block_label,
    //                 (i % 3) + 1,
    //             ));
    //         }

    //         script.push_str("unset multiplot\n");
    //         for i in 1..=d {
    //             script.push_str(&format!("unset label {}\n", i));
    //         }

    //         script
    //     }
}

//------------------------------------------------------------------------------

#[macro_export]
macro_rules! register_one {
    // If value is a number literal, convert to f32
    ($graph:expr, $name:expr, $val:literal) => {
        $graph.add_node($name, Box::new($crate::UGConst::new($val as Sample)));
    };
    // For all other expressions (e.g., full UGen constructors)
    ($graph:expr, $name:expr, $ugen:expr) => {
        $graph.add_node($name, Box::new($ugen));
    };
}

#[macro_export]
macro_rules! register_many {
    ($graph:expr, $( $name:expr => $ugen:expr ),+ $(,)?) => {
        $(
            $crate::register_one!($graph, $name, $ugen);
        )+
    };
}

#[macro_export]
macro_rules! connect_many {
    ($graph:expr, $( $src:literal -> $dst:literal ),+ $(,)?) => {
        $(
            $graph.connect($src, $dst);
        )+
    };
}

//------------------------------------------------------------------------------

// pub fn plot_graph_to_image(graph: &GenGraph, output: &str) -> std::io::Result<()> {
//     let script = graph.to_gnuplot(output.as_ref());
//     println!("OLD====================================");
//     println!("{}", &script);
//     let mut file = NamedTempFile::new()?;
//     write!(file, "{script}")?;
//     let script_path = file.path();
//     let status = Command::new("gnuplot").arg(script_path).status()?;

//     if !status.success() {
//         eprintln!("gnuplot failed with exit code: {:?}", status.code());
//     }

//     Ok(())
// }

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ModeRound, UGAsHz, UGClock, UGConst, UGLowPass, UGRound, UGSine, UnitRate,
    };

    #[test]
    fn test_gen_graph_describe_json_a() {
        let mut graph = GenGraph::new(44100.0, 128);

        graph.add_node("note", Box::new(UGConst::new(69.0))); // A4
        graph.add_node("conv", Box::new(UGAsHz::new(UnitRate::Midi)));
        graph.add_node("osc", Box::new(UGSine::new()));

        graph.connect("note.out", "conv.in");
        graph.connect("conv.out", "osc.freq");

        assert_eq!(
            graph.describe_json().to_string(),
            r#"[{"config":"value = 69.000","id":0,"inputs":[],"name":"note","outputs":[{"name":"out","value":0.0}],"type":"UGConst"},{"config":"mode = midi","id":1,"inputs":[{"connected_to":{"node":"note","output":"out"},"name":"in"}],"name":"conv","outputs":[{"name":"out","value":0.0}],"type":"UGAsHz"},{"config":null,"id":2,"inputs":[{"connected_to":{"node":"conv","output":"out"},"name":"freq"},{"default":0.0,"name":"phase"},{"default":-1.0,"name":"min"},{"default":1.0,"name":"max"}],"name":"osc","outputs":[{"name":"wave","value":0.0},{"name":"trigger","value":0.0}],"type":"UGSine"}]"#
        );
    }

    #[test]
    fn test_get_outputs_a() {
        let mut g = GenGraph::new(2000.0, 20);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "lpf" => UGLowPass::new(12.0),
            "fq" => 60,
            "r" => UGRound::new(3, ModeRound::Round),
        ];

        let outputs = g.get_outputs();
        let mut post: Vec<&String> = outputs.iter().map(|(k, _)| k).collect();
        post.sort();

        assert_eq!(post, vec!["clock.out", "fq.out", "lpf.out", "r.out"]);
    }
}
