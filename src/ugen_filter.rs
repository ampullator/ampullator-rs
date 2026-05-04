use crate::Sample;
use crate::UGen;

fn db_per_octave_to_poles(db: f32) -> usize {
    ((db / 6.0).round()).clamp(1.0, 12.0) as usize
}

/// Inner low-pass sample computation: state-variable cascade with resonance feedback.
/// Updates `state` and `z1` in place and returns the filtered sample.
#[inline]
fn low_pass_sample(x: Sample, g: f32, res: f32, state: &mut [Sample], z1: &mut Sample) -> Sample {
    let mut y = x - res * *z1;
    for s in state.iter_mut() {
        *s += g * (y - *s);
        y = *s;
    }
    *z1 = y;
    y
}

/// Inner high-pass sample computation: state-variable cascade with resonance feedback.
/// Updates `state` and `z1` in place and returns the filtered sample.
#[inline]
fn high_pass_sample(x: Sample, g: f32, res: f32, state: &mut [Sample], z1: &mut Sample) -> Sample {
    let mut y = x - res * *z1;
    for s in state.iter_mut() {
        *s += g * (y - *s);
        y -= *s;
    }
    *z1 = y;
    y
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

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["in".to_string(), "cutoff".to_string()])
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
    state: Vec<Sample>,
    z1: Sample,
}

impl UGLowPassQ {
    pub fn new(roll_off_db: f32) -> Self {
        let poles = db_per_octave_to_poles(roll_off_db);
        Self {
            state: vec![0.0; poles],
            z1: 0.0,
        }
    }
}

impl UGen for UGLowPassQ {
    fn type_name(&self) -> &'static str {
        "UGLowPassQ"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            vec![
                "in".to_string(),
                "cutoff".to_string(),
                "resonance".to_string(),
            ]
        })
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
            out[i] = low_pass_sample(x, g, res, &mut self.state, &mut self.z1);
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

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["in".to_string(), "cutoff".to_string()])
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
    state: Vec<Sample>,
    z1: Sample,
}

impl UGHighPassQ {
    pub fn new(roll_off_db: f32) -> Self {
        let poles = db_per_octave_to_poles(roll_off_db);
        Self {
            state: vec![0.0; poles],
            z1: 0.0,
        }
    }
}

impl UGen for UGHighPassQ {
    fn type_name(&self) -> &'static str {
        "UGHighPassQ"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            vec![
                "in".to_string(),
                "cutoff".to_string(),
                "resonance".to_string(),
            ]
        })
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
            out[i] = high_pass_sample(x, g, res, &mut self.state, &mut self.z1);
        }
    }
}

/// A low-pass filter with cutoff, resonance, and roll-off fixed at initialization.
/// Processes `channels` audio streams in parallel with the same filter parameters.
/// More efficient than `UGLowPassQ` when parameters do not vary at runtime.
///
/// Inputs: `in1` … `inN`. Outputs: `out1` … `outN`.
pub struct UGLowPassConst {
    cutoff: f32,
    resonance: f32,
    /// Per-channel filter state: integrator states + feedback z1.
    channel_state: Vec<(Vec<Sample>, Sample)>,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
}

impl UGLowPassConst {
    pub fn new(roll_off_db: f32, cutoff: f32, resonance: f32, channels: usize) -> Self {
        assert!(channels >= 1, "channels must be at least 1");
        let poles = db_per_octave_to_poles(roll_off_db);
        let channel_state = (0..channels)
            .map(|_| (vec![0.0f32; poles], 0.0f32))
            .collect();
        let input_refs = (1..=channels).map(|i| format!("in{i}")).collect();
        let output_refs = (1..=channels).map(|i| format!("out{i}")).collect();
        Self {
            cutoff: cutoff.clamp(1.0, f32::MAX),
            resonance: resonance.clamp(0.0, 1.0),
            channel_state,
            input_refs,
            output_refs,
        }
    }
}

impl UGen for UGLowPassConst {
    fn type_name(&self) -> &'static str {
        "UGLowPassConst"
    }

    fn input_names(&self) -> &[String] {
        &self.input_refs
    }

    fn output_names(&self) -> &[String] {
        &self.output_refs
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let n = match outputs.first() {
            Some(out) => out.len(),
            None => return,
        };
        let fc = self.cutoff.clamp(1.0, sample_rate / 2.0);
        let g = (2.0 * std::f32::consts::PI * fc / sample_rate).clamp(0.0, 1.0);
        let res = self.resonance;

        for (ch, (out, (state, z1))) in outputs
            .iter_mut()
            .zip(self.channel_state.iter_mut())
            .enumerate()
        {
            let input = inputs.get(ch).copied().unwrap_or(&[]);
            for i in 0..n {
                let x = input.get(i).copied().unwrap_or(0.0);
                out[i] = low_pass_sample(x, g, res, state, z1);
            }
        }
    }
}

/// A high-pass filter with cutoff, resonance, and roll-off fixed at initialization.
/// Processes `channels` audio streams in parallel with the same filter parameters.
/// More efficient than `UGHighPassQ` when parameters do not vary at runtime.
///
/// Inputs: `in1` … `inN`. Outputs: `out1` … `outN`.
pub struct UGHighPassConst {
    cutoff: f32,
    resonance: f32,
    /// Per-channel filter state: integrator states + feedback z1.
    channel_state: Vec<(Vec<Sample>, Sample)>,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
}

impl UGHighPassConst {
    pub fn new(roll_off_db: f32, cutoff: f32, resonance: f32, channels: usize) -> Self {
        assert!(channels >= 1, "channels must be at least 1");
        let poles = db_per_octave_to_poles(roll_off_db);
        let channel_state = (0..channels)
            .map(|_| (vec![0.0f32; poles], 0.0f32))
            .collect();
        let input_refs = (1..=channels).map(|i| format!("in{i}")).collect();
        let output_refs = (1..=channels).map(|i| format!("out{i}")).collect();
        Self {
            cutoff: cutoff.clamp(1.0, f32::MAX),
            resonance: resonance.clamp(0.0, 1.0),
            channel_state,
            input_refs,
            output_refs,
        }
    }
}

impl UGen for UGHighPassConst {
    fn type_name(&self) -> &'static str {
        "UGHighPassConst"
    }

    fn input_names(&self) -> &[String] {
        &self.input_refs
    }

    fn output_names(&self) -> &[String] {
        &self.output_refs
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let n = match outputs.first() {
            Some(out) => out.len(),
            None => return,
        };
        let fc = self.cutoff.clamp(1.0, sample_rate / 2.0);
        let g = (2.0 * std::f32::consts::PI * fc / sample_rate).clamp(0.0, 1.0);
        let res = self.resonance;

        for (ch, (out, (state, z1))) in outputs
            .iter_mut()
            .zip(self.channel_state.iter_mut())
            .enumerate()
        {
            let input = inputs.get(ch).copied().unwrap_or(&[]);
            for i in 0..n {
                let x = input.get(i).copied().unwrap_or(0.0);
                out[i] = high_pass_sample(x, g, res, state, z1);
            }
        }
    }
}

/// Compute normalized biquad peaking EQ coefficients (Audio EQ Cookbook, R. Bristow-Johnson).
/// Returns `(b0, b1, b2, a1, a2)` — all normalized by `a0`.
#[inline]
fn peaking_eq_coeffs(
    db_gain: f32,
    bw: f32,
    fc: f32,
    sample_rate: f32,
) -> (f32, f32, f32, f32, f32) {
    let a = 10.0_f32.powf(db_gain / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * fc / sample_rate;
    let sin_w0 = w0.sin().max(1e-6); // guard against division by zero near Nyquist
    let cos_w0 = w0.cos();
    let alpha = sin_w0 * (std::f32::consts::LN_2 / 2.0 * bw * w0 / sin_w0).sinh();

    let a0 = 1.0 + alpha / a;
    let b0 = (1.0 + alpha * a) / a0;
    let b1 = -2.0 * cos_w0 / a0;
    let b2 = (1.0 - alpha * a) / a0;
    let a1 = -2.0 * cos_w0 / a0;
    let a2 = (1.0 - alpha / a) / a0;
    (b0, b1, b2, a1, a2)
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

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            vec![
                "in".to_string(),
                "gain".to_string(),
                "bw".to_string(),
                "freq".to_string(),
            ]
        })
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
            let bw = bandwidth.get(i).copied().unwrap_or(1.0 / 3.0).max(0.001);
            let fc = freq
                .get(i)
                .copied()
                .unwrap_or(1000.0)
                .clamp(1.0, sample_rate * 0.5 - 1.0);

            let (b0, b1, b2, a1, a2) = peaking_eq_coeffs(db_gain, bw, fc, sample_rate);

            let y = b0 * x + b1 * self.x1 + b2 * self.x2 - a1 * self.y1 - a2 * self.y2;

            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;

            out[i] = y;
        }
    }
}

/// A parametric equalizer with gain, bandwidth, and center frequency fixed at initialization.
/// Uses the same biquad peaking EQ filter as `UGParametric`. Only the audio signal is a
/// signal input; the EQ parameters are constant across the lifetime of the node.
pub struct UGParametricConst {
    db_gain: f32,
    bw: f32,
    freq: f32,
    x1: Sample,
    x2: Sample,
    y1: Sample,
    y2: Sample,
}

impl UGParametricConst {
    pub fn new(db_gain: f32, bw: f32, freq: f32) -> Self {
        Self {
            db_gain,
            bw: bw.max(0.001),
            freq,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

impl UGen for UGParametricConst {
    fn type_name(&self) -> &'static str {
        "UGParametricConst"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["in".to_string()])
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs[0];
        let out = &mut outputs[0];

        let fc = self.freq.clamp(1.0, sample_rate * 0.5 - 1.0);
        let (b0, b1, b2, a1, a2) =
            peaking_eq_coeffs(self.db_gain, self.bw, fc, sample_rate);

        for i in 0..out.len() {
            let x = input[i];
            let y = b0 * x + b1 * self.x1 + b2 * self.x2 - a1 * self.y1 - a2 * self.y2;
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
    use crate::UGSine;
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
            "bw.out" -> "pq.bw",
            "freq.out" -> "pq.freq",
            "pq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0,
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
            "bw.out" -> "pq.bw",
            "freq.out" -> "pq.freq",
            "pq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.015, 0.029, 0.027, 0.024, 0.02, 0.015, 0.01, 0.005, -0.0, -0.005,
                -0.01, -0.014, -0.018, -0.021, -0.023, -0.024,
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
            "bw.out" -> "pq.bw",
            "freq.out" -> "pq.freq",
            "pq.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.985, -0.028, -0.025, -0.021, -0.017, -0.012, -0.007, -0.003, 0.002,
                0.006, 0.01, 0.013, 0.016, 0.018, 0.019, 0.019,
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

    //--------------------------------------------------------------------------
    #[test]
    fn test_parametric_const_passthrough() {
        // With gain = 0 dB the parametric EQ is transparent: output equals input.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "pqc" => UGParametricConst::new(0.0, 1.0 / 3.0, 60.0),
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "pqc.in",
            "pqc.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0,
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_parametric_const_boost_a() {
        // 6 dB boost at 60 Hz with 1/3-octave bandwidth.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "pqc" => UGParametricConst::new(6.0, 1.0 / 3.0, 60.0),
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "pqc.in",
            "pqc.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.015, 0.029, 0.027, 0.024, 0.02, 0.015, 0.01, 0.005, -0.0, -0.005,
                -0.01, -0.014, -0.018, -0.021, -0.023, -0.024,
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_parametric_const_cut_a() {
        // 6 dB cut at 60 Hz with 1/3-octave bandwidth.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "pqc" => UGParametricConst::new(-6.0, 1.0 / 3.0, 60.0),
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "pqc.in",
            "pqc.out" -> "r.in"
        ];
        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.985, -0.028, -0.025, -0.021, -0.017, -0.012, -0.007, -0.003, 0.002,
                0.006, 0.01, 0.013, 0.016, 0.018, 0.019, 0.019,
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_low_pass_const_matches_low_pass_q() {
        // UGLowPassConst with single channel should match UGLowPassQ with the
        // same constant cutoff and resonance inputs.
        let mut g_q = GenGraph::new(2000.0, 16);
        register_many![g_q,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "lpfq" => UGLowPassQ::new(12.0),
            "fq" => 60,
            "res" => 0.5_f32,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g_q,
            "clock.out" -> "lpfq.in",
            "fq.out" -> "lpfq.cutoff",
            "res.out" -> "lpfq.resonance",
            "lpfq.out" -> "r.in"
        ];
        g_q.process();
        let q_out = g_q.get_output_by_label("r.out");

        let mut g_c = GenGraph::new(2000.0, 16);
        register_many![g_c,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "lpfc" => UGLowPassConst::new(12.0, 60.0, 0.5, 1),
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g_c,
            "clock.out" -> "lpfc.in1",
            "lpfc.out1" -> "r.in"
        ];
        g_c.process();
        let c_out = g_c.get_output_by_label("r.out");

        assert_eq!(c_out, q_out);
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_low_pass_const_two_channels_independent() {
        // Two channels fed different signals should each be filtered independently
        // with the same filter parameters.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "sine" => UGSine::new(),
            "lpfc" => UGLowPassConst::new(12.0, 60.0, 0.5, 2),
            "r1" => UGRound::new(3, ModeRound::Round),
            "r2" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "lpfc.in1",
            "sine.wave" -> "lpfc.in2",
            "lpfc.out1" -> "r1.in",
            "lpfc.out2" -> "r2.in"
        ];
        g.process();

        // Each channel should produce filtered output (non-zero) independently.
        let out1 = g.get_output_by_label("r1.out");
        let out2 = g.get_output_by_label("r2.out");
        // Channels fed different signals must produce different outputs.
        assert_ne!(out1, out2);
        // Both outputs should be non-trivially non-zero (the filter has processed signal).
        assert!(out1.iter().any(|&v| v != 0.0));
        assert!(out2.iter().any(|&v| v != 0.0));
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_high_pass_const_matches_high_pass_q() {
        // UGHighPassConst with single channel should match UGHighPassQ with the
        // same constant cutoff and resonance inputs.
        let mut g_q = GenGraph::new(2000.0, 16);
        register_many![g_q,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "hpfq" => UGHighPassQ::new(12.0),
            "fq" => 60,
            "res" => 0.5_f32,
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g_q,
            "clock.out" -> "hpfq.in",
            "fq.out" -> "hpfq.cutoff",
            "res.out" -> "hpfq.resonance",
            "hpfq.out" -> "r.in"
        ];
        g_q.process();
        let q_out = g_q.get_output_by_label("r.out");

        let mut g_c = GenGraph::new(2000.0, 16);
        register_many![g_c,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "hpfc" => UGHighPassConst::new(12.0, 60.0, 0.5, 1),
            "r" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g_c,
            "clock.out" -> "hpfc.in1",
            "hpfc.out1" -> "r.in"
        ];
        g_c.process();
        let c_out = g_c.get_output_by_label("r.out");

        assert_eq!(c_out, q_out);
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_high_pass_const_two_channels_independent() {
        // Two channels fed different signals should each be filtered independently
        // with the same filter parameters.
        let mut g = GenGraph::new(2000.0, 16);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
            "sine" => UGSine::new(),
            "hpfc" => UGHighPassConst::new(12.0, 60.0, 0.5, 2),
            "r1" => UGRound::new(3, ModeRound::Round),
            "r2" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
            "clock.out" -> "hpfc.in1",
            "sine.wave" -> "hpfc.in2",
            "hpfc.out1" -> "r1.in",
            "hpfc.out2" -> "r2.in"
        ];
        g.process();

        let out1 = g.get_output_by_label("r1.out");
        let out2 = g.get_output_by_label("r2.out");
        assert_ne!(out1, out2);
        assert!(out1.iter().any(|&v| v != 0.0));
        assert!(out2.iter().any(|&v| v != 0.0));
    }
}
