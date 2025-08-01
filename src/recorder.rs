use crate::Sample;
use std::collections::HashMap;
// use crate::UGen;
use crate::GenGraph;
use std::collections::HashSet;
use std::path::Path;

use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

pub struct Recorder {
    sample_rate: f32,
    recorded: HashMap<String, Vec<Sample>>,
    output_names: Vec<String>,
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
            for label in &collected_labels {
                let buffer = graph.get_output_by_label(&label);
                recorded.get_mut(&*label).unwrap().extend_from_slice(buffer);
            }
        }

        // this seems to trim down to the total size
        for samples in recorded.values_mut() {
            samples.truncate(total_samples);
        }

        Self {
            sample_rate,
            recorded,
            output_names: graph.get_node_output_names(),
        }
    }

    //--------------------------------------------------------------------------
    pub fn get_shape(&self) -> (usize, usize) {
        let channels = self.recorded.len();
        let length = self.recorded.values().map(|v| v.len()).max().unwrap_or(0);
        (channels, length)
    }

    /// Given a fully-qualified label (node, output), return a `Sample` slice.
    pub fn get_output_by_label(&self, label: &str) -> &[Sample] {
        self.recorded
            .get(label)
            .expect(format!("No such label: {}", label).as_str())
    }

    //--------------------------------------------------------------------------
    pub fn to_gnuplot(&self, fp: &Path) -> String {
        let (d, _samples) = self.get_shape();
        let base_height_per_lane = 100; // pixels per lane
        let width = 800;
        let height = d * base_height_per_lane;

        let mut script = String::new();

        script.push_str(&format!(
            "set terminal pngcairo size {},{} background rgb '#12131E'\n",
            width, height
        ));
        script.push_str(&format!("set output '{}'\n\n", fp.display()));

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
set pointsize 0.5
unset xtics

# Color and style setup
do for [i=1:3] {
    set style line i lt 1 lw 1 pt 7 lc rgb '#5599ff'
}

# Multiplot setup
set multiplot
"#,
        );

        script.push_str(&format!("d = {d}\n"));
        script.push_str("margin = 0.04\n");
        script.push_str("height = 1.0 / d\n");
        script.push_str("pos = 1.0\n\n");
        script.push_str("label_x = 0.06\n");
        script.push_str("label_font = \",9\"\n\n");

        for (i, label) in self.output_names.iter().enumerate() {
            let values = self
                .recorded
                .get(label)
                .expect(format!("expected label {label} not found").as_str());
            let panel = i + 1;
            let block_label = label.replace(['.', '-', ' '], "_");

            // Data block
            script.push_str(&format!("${block_label} << EOD\n"));
            for v in values {
                script.push_str(&format!("{v}\n"));
            }
            script.push_str("EOD\n");

            script.push_str(&format!(
                r#"
# Panel {}
top = pos - margin * {}
bottom = pos - height + margin * 0.5
pos = pos - height
set tmargin screen top
set bmargin screen bottom
set label textcolor rgb '#c4c5bf'
set border lc rgb '#c4c5bf'
set grid lc rgb '#cccccc'
set label {} "{}" at screen label_x, screen (bottom + height / 2) center font label_font
plot ${} using 1 with linespoints linestyle {}
"#,
                panel,
                if i == 0 { 1.0 } else { 0.5 },
                panel,
                label,
                block_label,
                (i % 3) + 1,
            ));
        }

        script.push_str("unset multiplot\n");
        for i in 1..=d {
            script.push_str(&format!("unset label {i}\n"));
        }

        script
    }

    pub fn to_gnuplot_fp(&self, fp: &str) -> std::io::Result<()> {
        let script = self.to_gnuplot(fp.as_ref());
        let mut file = NamedTempFile::new()?;
        write!(file, "{script}")?;
        let script_path = file.path();
        let status = Command::new("gnuplot").arg(script_path).status()?;

        if !status.success() {
            eprintln!("gnuplot failed with exit code: {:?}", status.code());
        }

        Ok(())
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::connect_many;
    use crate::register_many;
    use crate::{ModeRound, UGClock, UGEnvAR, UGRound, UGSine, UnitRate};

    #[test]
    fn test_recorder_a() {
        let mut g = GenGraph::new(8.0, 10);
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

        g.process();
        // plot_graph_to_image(&g, "/tmp/ampullator-old.png");
        let r1 = Recorder::from_samples(g, None, 10);
        assert_eq!(r1.get_shape(), (5, 10));
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();
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

    #[test]
    fn test_recorder_c() {
        let mut g = GenGraph::new(10.0, 20);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
            "fq.out" -> "osc.freq",
            "osc.wave" -> "round.in"
        ];

        let r1 = Recorder::from_samples(g, None, 120);
        assert_eq!(r1.get_shape(), (4, 120));
        assert_eq!(
            r1.get_output_by_label("round.out"),
            vec![
                0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809, 0.5878, 0.309, -0.0,
                -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511, -0.809, -0.5878, -0.309,
                0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809, 0.5878, 0.309,
                -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511, -0.809, -0.5878,
                -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809, 0.5878,
                0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511, -0.809,
                -0.5878, -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809,
                0.5878, 0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511,
                -0.809, -0.5878, -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511,
                0.809, 0.5878, 0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0,
                -0.9511, -0.809, -0.5878, -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0,
                0.9511, 0.809, 0.5878, 0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511,
                -1.0, -0.9511, -0.809, -0.5878, -0.309, 0.0
            ]
        );
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();
    }
}
