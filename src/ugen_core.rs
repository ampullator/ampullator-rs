use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::Deserialize;
use serde::Serialize;

use crate::util::Sample;
use crate::util::UnitRate;
use crate::util::unit_rate_to_hz;

//------------------------------------------------------------------------------

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
        &["out"]
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs.first().copied().unwrap_or(&[]);
        let out = &mut outputs[0];
        for i in 0..out.len() {
            let x = input.get(i).copied().unwrap_or(0.0);
            out[i] = unit_rate_to_hz(x, self.mode, sample_rate)
        }
    }
}

//------------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        let input = inputs.first().copied().unwrap_or(&[]);
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
    input_refs: Vec<&'static str>,
}

impl UGSum {
    pub fn new(input_count: usize) -> Self {
        if input_count <= 1 {
            panic!("Input count should be greater than 1");
        }
        // input labels wil start with in1, ..., inN
        let input_labels: Vec<String> =
            (1..input_count + 1).map(|i| format!("in{i}")).collect();
        // println!("{:?}", input_labels);
        // Promote to 'static using Box::leak safely
        let input_refs: Vec<&'static str> = input_labels
            .iter()
            .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
            .collect();

        Self { input_refs }
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

        // TODO: use SIMD
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
        self.seed.map(|s| format!("seed = {s}"))
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let min_in = inputs.first().copied().unwrap_or(&[]);
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
        let freq_in = inputs.first().copied().unwrap_or(&[]);
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

/// Given a signal-controlled frequency, output an impulse.
pub struct UGTrigger {
    phase: f32,
}

impl UGTrigger {
    pub fn new() -> Self {
        Self { phase: 0.0 }
    }
}

impl UGen for UGTrigger {
    fn type_name(&self) -> &'static str {
        "UGTrigger"
    }
    fn input_names(&self) -> &[&'static str] {
        &["freq"]
    }
    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }
    fn default_input(&self, input_name: &str) -> Option<Sample> {
        if input_name == "freq" {
            Some(1.0)
        } else {
            None
        }
    }
    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let rate = inputs[0];
        let out = &mut outputs[0];

        out[0] = 1.0;

        for i in 1..out.len() {
            let hz = rate[i].max(0.0); // clamp negative rates to 0
            let phase_inc = hz / sample_rate;

            self.phase += phase_inc;
            if self.phase >= 1.0 {
                self.phase = 0.0;
            }
            out[i] = if self.phase < phase_inc { 1.0 } else { 0.0 };
        }
    }
}

//------------------------------------------------------------------------------

/// Given a constant rate determined by a value and a `UnitRate`, output impulses as long as the signal input is positive.
pub struct UGClock {
    value: Sample,
    mode: UnitRate,
    phase: Sample,
}

impl UGClock {
    pub fn new(value: Sample, mode: UnitRate) -> Self {
        Self {
            value,
            mode,
            phase: 1.0, // init to one to fire on first sample
        }
    }
}

impl UGen for UGClock {
    fn type_name(&self) -> &'static str {
        "UGClock"
    }

    fn input_names(&self) -> &[&'static str] {
        &["in"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "in" => Some(1.0),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("value = {}, mode = {:?}", self.value, self.mode))
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let enabled = inputs.first().copied().unwrap_or(&[]);
        let out = &mut outputs[0];
        let hz = unit_rate_to_hz(self.value, self.mode, sample_rate);
        let phase_inc = hz / sample_rate;

        for i in 0..out.len() {
            let on = enabled.get(i).copied().unwrap_or(1.0) > 0.5;
            if !on {
                out[i] = 0.0;
                continue;
            }

            self.phase += phase_inc;
            if self.phase >= 1.0 {
                self.phase = 0.0;
                out[i] = 1.0;
            } else {
                out[i] = 0.0;
            }
        }
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::connect_many;
    use crate::register_many;
    // use crate::Recorder;

    //--------------------------------------------------------------------------
    #[test]
    fn test_constant_a() {
        let c1 = UGConst::new(3.0);
        assert_eq!(c1.type_name(), "UGConst");

        let mut g = GenGraph::new(120.0, 8);
        g.add_node("c1", Box::new(c1));
        g.process();
        assert_eq!(
            g.get_output_by_label("c1.out"),
            vec![3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_sum_a() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "c1" => 3,
            "c2" => 2,
            "s1" => UGSum::new(2),
        ];
        connect_many![g,
        "c1.out" -> "s1.in1",
        "c2.out" -> "s1.in2",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_sine_a() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "c1" => 1.0,
            "osc1" => UGSine::new(),
            "r1" => UGRound::new(1, ModeRound::Round),
        ];
        connect_many![g,
        "c1.out" -> "osc1.freq",
        "osc1.wave" -> "r1.in",
        ];

        g.process();

        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![0.7, 1.0, 0.7, -0.0, -0.7, -1.0, -0.7, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_white_a() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
        "n1" => UGWhite::new(Some(42)),
        "r1" => UGRound::new(2, ModeRound::Round),
        ];
        g.connect("n1.out", "r1.in");
        g.process();

        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![-0.73, 0.05, -0.5, 0.09, 0.74, 0.27, 0.98, -0.19]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_clock_a() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "c1" => 4.0, // half the sampling rate
            "clock1" => UGTrigger::new(),
        ];
        connect_many![g,
        "c1.out" -> "clock1.freq",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("clock1.out"),
            vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn test_clock_b() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "c1" => 4.0, // half the sampling rate
            "x" => UGAsHz::new(UnitRate::Samples),
            "clock1" => UGTrigger::new(),
        ];
        connect_many![g,
        "c1.out" -> "x.in",
        "x.out" -> "clock1.freq",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("clock1.out"),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn test_clock_c() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "c1" => 3.0, // half the sampling rate
            "x" => UGAsHz::new(UnitRate::Samples),
            "clock1" => UGTrigger::new(),
        ];
        connect_many![g,
        "c1.out" -> "x.in",
        "x.out" -> "clock1.freq",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("clock1.out"),
            vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0]
        );
    }
}
