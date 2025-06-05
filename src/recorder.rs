use std::collections::HashMap;
use crate::Sample;
use crate::UGen;


pub struct Recorder {
    graph: GenGraph,
    output_labels: Option<Vec<String>>,
    recorded: HashMap<String, Vec<Sample>>,
}

impl Recorder {
    pub fn new(graph: GenGraph, output_labels: Option<Vec<String>>) -> Self {
        // validate that output labels are in this Graph
        Self {
            graph,
            output_labels,
            recorded: HashMap::new(),
        }
    }

    // pub fn record(&mut self, samples: usize) {

    //     for _ in 0..samples {
    //         self.graph.process();
    //         for (name, signal) in self.graph.get_all_outputs() {
    //             if self.output_labels.as_ref().map_or(true, |labels| labels.contains(&name)) {
    //                 self.recorded.entry(name.clone()).or_default().push(*signal.last().unwrap_or(&0.0));
    //             }
    //         }
    //     }
    // }

}



