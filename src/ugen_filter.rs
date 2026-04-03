use crate::Sample;
use crate::UGen;

fn db_per_octave_to_poles(db: f32) -> usize {
    ((db / 6.0).round()).clamp(1.0, 12.0) as usize
}

/// A low pass filter with variable cutoff frequency. Rolloff configurable at initialization.
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
            // println!("g: {g:?}");

            let mut y = x;
            for p in 0..self.poles {
                self.state[p] += g * (y - self.state[p]);
                y = self.state[p];
            }

            out[i] = y;
        }
    }
}

/// A low pass filter with variable cutoff and resonance. Roll-off configuraable at initialization.
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

/// A high pass filter with variable cutoff frequency. Rolloff configurable at initialization.
pub struct UGHighPass {
    poles: usize,
    state: Vec<Sample>,
}

impl UGHighPass {
    pub fn new(roll_off_db: f32) -> Self {
        let poles = db_per_octave_to_poles(roll_off_db);
        Self {
            poles,
            state: vec![0.0; poles],
        }
    }
}

impl UGen for UGHighPass {
    fn type_name(&self) -> &'static str {
        "UGHighPass"
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

            let mut y = x;
            for p in 0..self.poles {
                self.state[p] += g * (y - self.state[p]);
                y -= self.state[p];
            }

            out[i] = y;
        }
    }
}

/// A high pass filter with variable cutoff and resonance. Roll-off configurable at initialization.
pub struct UGHighPassQ {
    poles: usize,
    state: Vec<Sample>,
    z1: Sample,
}

impl UGHighPassQ {
    pub fn new(roll_off_db: f32) -> Self {
        let poles = db_per_octave_to_poles(roll_off_db);
        Self {
            poles,
            state: vec![0.0; poles],
            z1: 0.0,
        }
    }
}

impl UGen for UGHighPassQ {
    fn type_name(&self) -> &'static str {
        "UGHighPassQ"
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
                y -= self.state[p];
            }

            self.z1 = y; // feedback storage
            out[i] = y;
        }
    }
}

/// A fully sweepable parametric equalizer with variable gain, bandwidth, and center frequency.
/// Uses a biquad peaking EQ filter (Audio EQ Cookbook). No initialization arguments;
/// all parameters are controlled via signal inputs.
pub struct UGParametric {
    x1: Sample,
    x2: Sample,
    y1: Sample,
    y2: Sample,
}

impl UGParametric {
    pub fn new() -> Self {
        Self {
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

impl Default for UGParametric {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGParametric {
    fn type_name(&self) -> &'static str {
        "UGParametric"
    }

    fn input_names(&self) -> &[&'static str] {
        &["in", "gain", "bandwidth", "freq"]
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
        let gain = inputs.get(1).copied().unwrap_or(&[]);
        let bandwidth = inputs.get(2).copied().unwrap_or(&[]);
        let freq = inputs.get(3).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let x = input[i];
            let db_gain = gain.get(i).copied().unwrap_or(0.0);
            let bw = bandwidth
                .get(i)
                .copied()
                .unwrap_or(1.0 / 3.0)
                .max(0.001);
            let fc = freq
                .get(i)
                .copied()
                .unwrap_or(1000.0)
                .clamp(1.0, sample_rate * 0.5 - 1.0);

            // Biquad peaking EQ coefficients (Audio EQ Cookbook, R. Bristow-Johnson)
            let a = 10.0_f32.powf(db_gain / 40.0);
            let w0 = 2.0 * std::f32::consts::PI * fc / sample_rate;
            let sin_w0 = w0.sin().max(1e-6); // guard against division by zero near Nyquist
            let cos_w0 = w0.cos();
            let alpha =
                sin_w0 * (std::f32::consts::LN_2 / 2.0 * bw * w0 / sin_w0).sinh();

            let a0 = 1.0 + alpha / a;
            let b0 = (1.0 + alpha * a) / a0;
            let b1 = -2.0 * cos_w0 / a0;
            let b2 = (1.0 - alpha * a) / a0;
            let a1 = -2.0 * cos_w0 / a0;
            let a2 = (1.0 - alpha / a) / a0;

            let y =
                b0 * x + b1 * self.x1 + b2 * self.x2 - a1 * self.y1 - a2 * self.y2;

            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;

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
        let mut g = GenGraph::new(2000.0, 16);
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
            g.get_output_by_label("r.out"),
            vec![
                0.036, 0.058, 0.07, 0.076, 0.077, 0.075, 0.071, 0.066, 0.06, 0.054,
                0.048, 0.043, 0.038, 0.033, 0.029, 0.025
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_high_pass_a() {
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "hpf" => UGHighPass::new(12.0),
            "fq" => 60,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "hpf.in",
            "fq.out" -> "hpf.cutoff",
            "hpf.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.659, -0.248, -0.178, -0.126, -0.086, -0.058, -0.037, -0.021, -0.011,
                -0.003, 0.002, 0.005, 0.007, 0.008, 0.008, 0.008
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_parametric_passthrough() {
        // With gain = 0 dB the parametric EQ is transparent: output equals input.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "pq" => UGParametric::new(),
            "gain" => 0.0_f32,
            "bw" => 0.333_f32,
            "freq" => 60.0_f32,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "pq.in",
            "gain.out" -> "pq.gain",
            "bw.out" -> "pq.bandwidth",
            "freq.out" -> "pq.freq",
            "pq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_parametric_boost_a() {
        // 6 dB boost at 60 Hz with 1/3-octave bandwidth.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "pq" => UGParametric::new(),
            "gain" => 6.0_f32,
            "bw" => 0.333_f32,
            "freq" => 60.0_f32,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "pq.in",
            "gain.out" -> "pq.gain",
            "bw.out" -> "pq.bandwidth",
            "freq.out" -> "pq.freq",
            "pq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.015, 0.029, 0.027, 0.024, 0.02, 0.015, 0.01, 0.005,
                -0.0, -0.005, -0.01, -0.014, -0.018, -0.021, -0.023, -0.024,
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_parametric_cut_a() {
        // 6 dB cut at 60 Hz with 1/3-octave bandwidth.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "pq" => UGParametric::new(),
            "gain" => -6.0_f32,
            "bw" => 0.333_f32,
            "freq" => 60.0_f32,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "pq.in",
            "gain.out" -> "pq.gain",
            "bw.out" -> "pq.bandwidth",
            "freq.out" -> "pq.freq",
            "pq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.985, -0.028, -0.025, -0.021, -0.017, -0.012, -0.007, -0.003,
                0.002, 0.006, 0.01, 0.013, 0.016, 0.018, 0.019, 0.019,
            ]
        );
    }

    #[test]
    fn test_high_pass_q_a() {
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "hpfq" => UGHighPassQ::new(12.0),
            "fq" => 60,
            "res" => 0.5_f32,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "hpfq.in",
            "fq.out" -> "hpfq.cutoff",
            "res.out" -> "hpfq.resonance",
            "hpfq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.659, -0.465, 0.057, -0.143, -0.032, -0.06, -0.031, -0.03, -0.018,
                -0.013, -0.008, -0.004, -0.001, 0.002, 0.004, 0.005
            ]
        );
    }
}
