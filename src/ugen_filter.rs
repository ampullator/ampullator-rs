use crate::Sample;
use crate::UGen;

fn db_per_octave_to_poles(db: f32) -> usize {
    ((db / 6.0).round()).clamp(1.0, 12.0) as usize
}

pub struct UGLowPass {
    poles: usize,
    state: Vec<Sample>,
}

impl UGLowPass {
    pub fn new(roll_off_db: f32) -> Self {
        let poles = db_per_octave_to_poles(roll_off_db);
        Self {
            poles,
            state: vec![0.0; poles],
        }
    }
}

impl UGen for UGLowPass {
    fn type_name(&self) -> &'static str {
        "UGLowPass"
    }

    fn input_names(&self) -> &[&'static str] {
        &["in", "cutoff"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs[0];
        let cutoff = inputs.get(1).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let x = input[i];
            let fc = cutoff
                .get(i)
                .copied()
                .unwrap_or(1000.0)
                .clamp(1.0, sample_rate / 2.0);
            let g = (2.0 * std::f32::consts::PI * fc / sample_rate).clamp(0.0, 1.0);
            println!("g: {:?}", g);

            let mut y = x;
            for p in 0..self.poles {
                self.state[p] += g * (y - self.state[p]);
                y = self.state[p];
            }

            out[i] = y;
        }
    }
}

pub struct UGLowPassQ {
    poles: usize,
    state: Vec<Sample>,
    z1: Sample,
}

impl UGLowPassQ {
    pub fn new(roll_off_db: f32) -> Self {
        let poles = db_per_octave_to_poles(roll_off_db);
        Self {
            poles,
            state: vec![0.0; poles],
            z1: 0.0,
        }
    }
}

impl UGen for UGLowPassQ {
    fn type_name(&self) -> &'static str {
        "UGLowPassQ"
    }

    fn input_names(&self) -> &[&'static str] {
        &["in", "cutoff", "resonance"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs[0];
        let cutoff = inputs.get(1).copied().unwrap_or(&[]);
        let resonance = inputs.get(2).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let x = input[i];
            let fc = cutoff
                .get(i)
                .copied()
                .unwrap_or(1000.0)
                .clamp(1.0, sample_rate / 2.0);
            let res = resonance.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);

            let g = (2.0 * std::f32::consts::PI * fc / sample_rate).clamp(0.0, 1.0);
            let mut y = x - res * self.z1;

            for p in 0..self.poles {
                self.state[p] += g * (y - self.state[p]);
                y = self.state[p];
            }

            self.z1 = y; // feedback storage
            out[i] = y;
        }
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::ModeRound;
    use crate::UGClock;
    use crate::UGRound;
    use crate::UnitRate;
    use crate::connect_many;
    use crate::register_many;
    // use crate::plot_graph_to_image;

    //--------------------------------------------------------------------------
    #[test]
    fn test_low_pass_a() {
        let mut g = GenGraph::new(2000.0, 20);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "lpf" => UGLowPass::new(12.0),
            "fq" => 60,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
        "clock.out" -> "lpf.in",
        "fq.out" -> "lpf.cutoff",
        "lpf.out" -> "r.in"
        ];
        g.process();

        // plot_graph_to_image(&g, "/tmp/ampullator.png").unwrap();

        assert_eq!(
            g.get_output_named("r.out"),
            vec![
                0.036, 0.058, 0.07, 0.076, 0.077, 0.075, 0.071, 0.066, 0.06, 0.054,
                0.048, 0.043, 0.038, 0.033, 0.029, 0.025, 0.021, 0.018, 0.016, 0.013
            ]
        );
    }
}
