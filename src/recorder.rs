use std::collections::HashMap;
use crate::Sample;
use crate::UGen;
use std::collections::HashSet;
use crate::GenGraph;

pub struct Recorder {
    sample_rate: f32,
    recorded: HashMap<String, Vec<Sample>>,
}

impl Recorder {

    pub fn from_samples(
        mut graph: GenGraph,
        output_labels: Option<Vec<String>>,
        total_samples: usize,
    ) -> Self {
        let sample_rate = graph.sample_rate;
        let buffer_size = graph.buffer_size;
        let mut recorded: HashMap<String, Vec<Sample>> = HashMap::new();
        let mut collected_labels: HashSet<String> = HashSet::new();

        match output_labels {
            Some(ref labels) => {
                for label in labels {
                    recorded.insert(label.clone(), Vec::with_capacity(total_samples));
                    collected_labels.insert(label.clone());
                }
            }
            None => {
                for (label, _) in graph.get_outputs() {
                    recorded.insert(label.clone(), Vec::with_capacity(total_samples));
                    collected_labels.insert(label);
                }
            }
        }

        let iterations = (total_samples + buffer_size - 1) / buffer_size;

        for _ in 0..iterations {
            graph.process();
            for (label, buffer) in graph.get_outputs() {
                if collected_labels.contains(&label) {
                    recorded
                        .get_mut(&label)
                        .unwrap()
                        .extend_from_slice(buffer);
                }
            }
        }

        for samples in recorded.values_mut() {
            samples.truncate(total_samples);
        }

        Self { sample_rate, recorded }
    }

    //--------------------------------------------------------------------------
    pub fn get_shape(&self) -> (usize, usize) {
        let channels = self.recorded.len();

        let length = self
            .recorded
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(0);

        (channels, length)
    }
}




//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UGRound, ModeRound, UnitRate, UGClock, UGEnvAR};
    use crate::GenGraph;
    use crate::connect_many;
    use crate::register_many;

    #[test]
    fn test_recorder_a() {

        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(16.0, UnitRate::Samples),
            "env" => UGEnvAR::new(),
            "a" => 4,
            "r" => 8,
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
        "clock.out" -> "env.trigger",
        "a.out" -> "env.attack_dur",
        "r.out" -> "env.release_dur",
        "env.out" -> "round.in"
        ];


        let r1 = Recorder::from_samples(g, None, 120);
        assert_eq!(r1.get_shape(), (5, 120));

    }


    #[test]
    fn test_recorder_b() {

        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(16.0, UnitRate::Samples),
            "env" => UGEnvAR::new(),
            "a" => 4,
            "r" => 8,
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
        "clock.out" -> "env.trigger",
        "a.out" -> "env.attack_dur",
        "r.out" -> "env.release_dur",
        "env.out" -> "round.in"
        ];


        let output_labels = Some(vec!["round.out".to_string()]);
        let r1 = Recorder::from_samples(g, output_labels, 120);
        assert_eq!(r1.get_shape(), (1, 120));

    }
}
