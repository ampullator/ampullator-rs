use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::str::FromStr;

use crate::util::Sample;


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
