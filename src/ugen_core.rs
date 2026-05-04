use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::Deserialize;
use serde::Serialize;
use wide::{CmpNe, f32x8};

use crate::util::Sample;
use crate::util::UnitRate;
use crate::util::unit_rate_to_hz;

/// Load 8 contiguous f32 values starting at `offset` from `slice` into an `f32x8` SIMD lane.
/// The caller must ensure `slice.len() >= offset + 8`.
/// On processors without SIMD support, `wide::f32x8` automatically falls back to scalar
/// operations, preserving correctness on all platforms.
#[inline(always)]
fn simd_load(slice: &[f32], offset: usize) -> f32x8 {
    f32x8::from(<[f32; 8]>::try_from(&slice[offset..offset + 8]).unwrap())
}

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
    fn input_names(&self) -> &[String];
    fn output_names(&self) -> &[String];
    fn default_input(&self, _input_name: &str) -> Option<Sample> {
        None
    }
    fn describe_config(&self) -> Option<String> {
        None
    }
    fn first_input(&self) -> Option<&str> {
        self.input_names().first().map(|s| s.as_str())
    }
    fn first_output(&self) -> Option<&str> {
        self.output_names().first().map(|s| s.as_str())
    }
    /// Return the first `n` input names if this UGen has at least `n` inputs,
    /// or `None` if it has fewer than `n`.  Used by the `&>` multi-signal
    /// operator to validate that the destination has enough inputs before
    /// wiring source outputs in bulk.
    fn get_n_inputs(&self, n: usize) -> Option<Vec<&str>> {
        let inputs = self.input_names();
        if inputs.len() >= n {
            Some(inputs[..n].iter().map(|s| s.as_str()).collect())
        } else {
            None
        }
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

    fn input_names(&self) -> &[String] {
        &[]
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
    }

    fn process(
        &mut self,
        _inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let out = &mut outputs[0];
        let chunks = out.len() / 8;
        let v = f32x8::splat(self.value);
        for c in 0..chunks {
            let i = c * 8;
            out[i..i + 8].copy_from_slice(&v.to_array());
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
        let input = inputs.first().copied().unwrap_or(&[]);
        let out = &mut outputs[0];
        let chunks = out.len() / 8;
        let zero = f32x8::splat(0.0);

        //  zero-safe divide for Seconds and Samples works by bitwise-ANDing the computed reciprocal with the simd_ne mask — positions where the input is zero have all-zero mask bits, which zero out the Inf result cleanly.

        // NOTE: util::unit_rate_to_hz provides an element-wise implementation.
        match self.mode {
            UnitRate::Hz => {
                for c in 0..chunks {
                    let i = c * 8;
                    out[i..i + 8].copy_from_slice(&simd_load(input, i).to_array());
                }
            }
            UnitRate::Seconds => {
                for c in 0..chunks {
                    let i = c * 8;
                    let x = simd_load(input, i);
                    let result = x.simd_ne(zero) & (f32x8::splat(1.0) / x);
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            UnitRate::Samples => {
                let sr = f32x8::splat(sample_rate);
                for c in 0..chunks {
                    let i = c * 8;
                    let x = simd_load(input, i);
                    let result = x.simd_ne(zero) & (sr / x);
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            UnitRate::Midi => {
                let base = f32x8::splat(2.0);
                let a = f32x8::splat(440.0);
                let offset = f32x8::splat(69.0);
                let inv12 = f32x8::splat(1.0 / 12.0);
                for c in 0..chunks {
                    let i = c * 8;
                    let x = simd_load(input, i);
                    let result = a * base.pow_f32x8((x - offset) * inv12);
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            UnitRate::Bpm => {
                let inv60 = f32x8::splat(1.0 / 60.0);
                for c in 0..chunks {
                    let i = c * 8;
                    let x = simd_load(input, i);
                    out[i..i + 8].copy_from_slice(&(x * inv60).to_array());
                }
            }
        }
    }
}

//------------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, Serialize, Deserialize, strum::EnumIter, strum::Display)]
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
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs[0];
        let out = &mut outputs[0];
        let chunks = out.len() / 8;
        let factor_v = f32x8::splat(self.factor);
        match self.mode {
            ModeRound::Round => {
                for c in 0..chunks {
                    let i = c * 8;
                    let result = (simd_load(input, i) * factor_v).round() / factor_v;
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            ModeRound::Floor => {
                for c in 0..chunks {
                    let i = c * 8;
                    let result = (simd_load(input, i) * factor_v).floor() / factor_v;
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            ModeRound::Ceil => {
                for c in 0..chunks {
                    let i = c * 8;
                    let result = (simd_load(input, i) * factor_v).ceil() / factor_v;
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGFloor;

impl UGFloor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UGFloor {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGFloor {
    fn type_name(&self) -> &'static str {
        "UGFloor"
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
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs[0];
        let out = &mut outputs[0];
        let chunks = out.len() / 8;
        for c in 0..chunks {
            let i = c * 8;
            let result = simd_load(input, i).floor();
            out[i..i + 8].copy_from_slice(&result.to_array());
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGCeil;

impl UGCeil {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UGCeil {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGCeil {
    fn type_name(&self) -> &'static str {
        "UGCeil"
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
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs[0];
        let out = &mut outputs[0];
        let chunks = out.len() / 8;
        for c in 0..chunks {
            let i = c * 8;
            let result = simd_load(input, i).ceil();
            out[i..i + 8].copy_from_slice(&result.to_array());
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGSum {
    input_refs: Vec<String>,
}

impl UGSum {
    pub fn new(inputs: usize) -> Self {
        if inputs <= 1 {
            panic!("Input count should be greater than 1");
        }
        let input_refs: Vec<String> = (1..inputs + 1).map(|i| format!("in{i}")).collect();

        Self { input_refs }
    }
}

impl UGen for UGSum {
    fn type_name(&self) -> &'static str {
        "UGSum"
    }

    fn input_names(&self) -> &[String] {
        &self.input_refs
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
        let chunks = len / 8;

        match inputs.len() {
            2 => {
                let a = inputs[0];
                let b = inputs[1];
                for c in 0..chunks {
                    let i = c * 8;
                    let result = simd_load(a, i) + simd_load(b, i);
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            _ => {
                for c in 0..chunks {
                    let i = c * 8;
                    let mut acc = f32x8::splat(0.0_f32);
                    for input in inputs {
                        acc += simd_load(input, i);
                    }
                    out[i..i + 8].copy_from_slice(&acc.to_array());
                }
            }
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGMult {
    input_refs: Vec<String>,
}

impl UGMult {
    pub fn new(inputs: usize) -> Self {
        if inputs <= 1 {
            panic!("Input count should be greater than 1");
        }
        let input_refs: Vec<String> = (1..inputs + 1).map(|i| format!("in{i}")).collect();

        Self { input_refs }
    }
}

impl UGen for UGMult {
    fn type_name(&self) -> &'static str {
        "UGMult"
    }

    fn input_names(&self) -> &[String] {
        &self.input_refs
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
        let chunks = len / 8;

        match inputs.len() {
            2 => {
                let a = inputs[0];
                let b = inputs[1];
                for c in 0..chunks {
                    let i = c * 8;
                    let result = simd_load(a, i) * simd_load(b, i);
                    out[i..i + 8].copy_from_slice(&result.to_array());
                }
            }
            _ => {
                for c in 0..chunks {
                    let i = c * 8;
                    let mut acc = f32x8::splat(1.0_f32);
                    for input in inputs {
                        acc *= simd_load(input, i);
                    }
                    out[i..i + 8].copy_from_slice(&acc.to_array());
                }
            }
        }
    }
}

//------------------------------------------------------------------------------

/// Convert a linear amplitude control value in `[0, 1]` to a gain factor using
/// logarithmic (perceptual) scaling.
///
/// Equal linear steps in the control produce equal perceived (dB) changes in
/// the output: `gain = 1000^(level - 1)`.  At `level = 0` the gain is
/// exactly `0.0` (silence); at `level = 1` the gain is `1.0` (unity).
/// Common values are short-circuited to avoid the cost of `powf`.
#[inline]
fn amplitude_to_gain(level: f32) -> f32 {
    if level <= 0.0 {
        0.0
    } else if level == 1.0 {
        1.0
    } else if level == 0.5 {
        // 1000^(0.5 - 1) = 1000^(-0.5) = 1/sqrt(1000)
        const GAIN_HALF: f32 = 0.031_622_776;
        GAIN_HALF
    } else {
        1000.0_f32.powf(level - 1.0)
    }
}

/// Distribute a single sample `x` across adjacent output channels using linear
/// (equal-power) panning and **accumulate** into the output buffers at index `i`.
///
/// `pair_pos` is the desired pan position expressed as a continuous channel
/// index in the range `[0, outputs.len() - 1]`.  Values are clamped to that
/// range before use.
#[inline]
fn pan_linear_accumulate(
    x: Sample,
    pair_pos: Sample,
    outputs: &mut [&mut [Sample]],
    i: usize,
) {
    let output_count = outputs.len();
    let pair_pos = pair_pos.clamp(0.0, output_count as Sample - 1.0);
    let left_index = pair_pos.floor() as usize;

    if left_index >= output_count - 1 {
        outputs[output_count - 1][i] += x;
        return;
    }

    let pair_pan = pair_pos - left_index as f32;
    let angle = pair_pan * std::f32::consts::FRAC_PI_2;
    let (left, right) = outputs.split_at_mut(left_index + 1);
    left[left_index][i] += x * angle.cos();
    right[0][i] += x * angle.sin();
}

//------------------------------------------------------------------------------

pub struct UGPan {
    output_refs: Vec<String>,
    default_pan: Sample,
}

impl UGPan {
    pub fn new(outputs: usize, pan: Sample) -> Self {
        if outputs < 2 {
            panic!("Output count should be greater than 1");
        }
        let output_refs: Vec<String> =
            (1..outputs + 1).map(|i| format!("out{i}")).collect();

        Self {
            output_refs,
            default_pan: pan,
        }
    }
}

impl Default for UGPan {
    fn default() -> Self {
        Self::new(2, 0.5)
    }
}

impl UGen for UGPan {
    fn type_name(&self) -> &'static str {
        "UGPan"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["in".to_string(), "pan".to_string()])
    }

    fn output_names(&self) -> &[String] {
        &self.output_refs
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "pan" => Some(self.default_pan),
            _ => None,
        }
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let input = inputs.first().copied().unwrap_or(&[]);
        let pan = inputs.get(1).copied().unwrap_or(&[]);
        let output_count = outputs.len();
        if output_count == 0 {
            return;
        }
        for out in outputs.iter_mut() {
            out.fill(0.0);
        }
        let n = outputs[0].len();

        for i in 0..n {
            let x = input.get(i).copied().unwrap_or(0.0);
            let pair_pos = pan.get(i).copied().unwrap_or(self.default_pan);
            pan_linear_accumulate(x, pair_pos, outputs, i);
        }
    }
}

//------------------------------------------------------------------------------

pub struct UGMixLinear {
    input_count: usize,
    output_count: usize,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
}

impl UGMixLinear {
    pub fn new(inputs: usize, outputs: usize) -> Self {
        if inputs < 1 {
            panic!("Input count should be at least 1");
        }
        if outputs < 2 {
            panic!("Output count should be at least 2");
        }
        let mut input_labels: Vec<String> = Vec::with_capacity(inputs * 3);
        for i in 1..=inputs {
            input_labels.push(format!("in{i}"));
            input_labels.push(format!("pan{i}"));
            input_labels.push(format!("level{i}"));
        }
        let input_refs = input_labels;

        let output_refs: Vec<String> = (1..=outputs).map(|i| format!("out{i}")).collect();

        Self {
            input_count: inputs,
            output_count: outputs,
            input_refs,
            output_refs,
        }
    }
}

impl Default for UGMixLinear {
    fn default() -> Self {
        Self::new(2, 2)
    }
}

impl UGen for UGMixLinear {
    fn type_name(&self) -> &'static str {
        "UGMixLinear"
    }

    fn input_names(&self) -> &[String] {
        &self.input_refs
    }

    fn output_names(&self) -> &[String] {
        &self.output_refs
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        if input_name.starts_with("level") {
            Some(1.0)
        } else if input_name.starts_with("pan") {
            Some((self.output_count as Sample - 1.0) * 0.5)
        } else {
            None
        }
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let output_count = outputs.len();
        if output_count == 0 {
            return;
        }
        for out in outputs.iter_mut() {
            out.fill(0.0);
        }
        let n = outputs[0].len();
        let default_pan = (output_count as Sample - 1.0) * 0.5;

        for ch in 0..self.input_count {
            let base = ch * 3;
            let in_sig = inputs.get(base).copied().unwrap_or(&[]);
            let in_pan = inputs.get(base + 1).copied().unwrap_or(&[]);
            let in_level = inputs.get(base + 2).copied().unwrap_or(&[]);

            for i in 0..n {
                let x = in_sig.get(i).copied().unwrap_or(0.0);
                let pan = in_pan.get(i).copied().unwrap_or(default_pan);
                let level = in_level.get(i).copied().unwrap_or(1.0);
                let gain = amplitude_to_gain(level);
                pan_linear_accumulate(x * gain, pan, outputs, i);
            }
        }
    }
}

//------------------------------------------------------------------------------

/// Scale one or more audio channels by a shared amplitude control using
/// perceptual (logarithmic) gain mapping.
///
/// Inputs: `in1` … `inN` (one per channel), `level` (amplitude control 0–1).
/// Outputs: `out1` … `outN` (each channel scaled independently by the same gain).
///
/// The gain formula is identical to the one used by `UGMixLinear`:
/// `gain = 1000^(level - 1)`, clamped to `0` when `level ≤ 0`.
pub struct UGFade {
    channels: usize,
    level: Sample,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
}

impl UGFade {
    pub fn new(channels: usize, level: Sample) -> Self {
        if channels < 1 {
            panic!("Channel count should be at least 1");
        }
        let mut input_labels: Vec<String> = Vec::with_capacity(channels + 1);
        for i in 1..=channels {
            input_labels.push(format!("in{i}"));
        }
        input_labels.push("level".to_string());
        let input_refs = input_labels;

        let output_refs: Vec<String> =
            (1..=channels).map(|i| format!("out{i}")).collect();

        Self {
            channels,
            level,
            input_refs,
            output_refs,
        }
    }
}

impl Default for UGFade {
    fn default() -> Self {
        Self::new(1, 1.0)
    }
}

impl UGen for UGFade {
    fn type_name(&self) -> &'static str {
        "UGFade"
    }

    fn input_names(&self) -> &[String] {
        &self.input_refs
    }

    fn output_names(&self) -> &[String] {
        &self.output_refs
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        if input_name == "level" {
            Some(self.level)
        } else {
            None
        }
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let n = match outputs.first() {
            Some(out) => out.len(),
            None => return,
        };
        // Input layout: in1…inN occupy indices 0..channels, level is at channels.
        let level_input_index = self.channels;
        let in_level = inputs.get(level_input_index).copied().unwrap_or(&[]);

        // Fast paths for the most common configurations avoid outer-loop overhead.
        // In all paths gain is computed once per sample, then applied across channels.
        if self.channels == 1 {
            let in_sig = inputs.first().copied().unwrap_or(&[]);
            let out = &mut outputs[0];
            for i in 0..n {
                let level = in_level.get(i).copied().unwrap_or(self.level);
                let gain = amplitude_to_gain(level);
                out[i] = in_sig.get(i).copied().unwrap_or(0.0) * gain;
            }
            return;
        }

        if self.channels == 2 {
            let (out01, _) = outputs.split_at_mut(2);
            let in0 = inputs.first().copied().unwrap_or(&[]);
            let in1 = inputs.get(1).copied().unwrap_or(&[]);
            #[allow(clippy::needless_range_loop)]
            for i in 0..n {
                let gain =
                    amplitude_to_gain(in_level.get(i).copied().unwrap_or(self.level));
                out01[0][i] = in0.get(i).copied().unwrap_or(0.0) * gain;
                out01[1][i] = in1.get(i).copied().unwrap_or(0.0) * gain;
            }
            return;
        }

        for i in 0..n {
            let gain = amplitude_to_gain(in_level.get(i).copied().unwrap_or(self.level));
            for (ch, out) in outputs.iter_mut().enumerate().take(self.channels) {
                let x = inputs
                    .get(ch)
                    .copied()
                    .unwrap_or(&[])
                    .get(i)
                    .copied()
                    .unwrap_or(0.0);
                out[i] = x * gain;
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
            seed, // original user-provided seed
        }
    }
}

impl UGen for UGWhite {
    fn type_name(&self) -> &'static str {
        "UGWhite"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["min".to_string(), "max".to_string()])
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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
        let n = out.len();

        match (min_in.len() >= n, max_in.len() >= n) {
            // most comon case
            (false, false) => {
                let (min, max) = (self.default_min, self.default_max);
                for v in out.iter_mut() {
                    *v = self.rng.random_range(min..=max);
                }
            }
            (true, false) => {
                let max = self.default_max;
                for i in 0..n {
                    out[i] = self.rng.random_range(min_in[i]..=max);
                }
            }
            (false, true) => {
                let min = self.default_min;
                for i in 0..n {
                    out[i] = self.rng.random_range(min..=max_in[i]);
                }
            }
            (true, true) => {
                for i in 0..n {
                    out[i] = self.rng.random_range(min_in[i]..=max_in[i]);
                }
            }
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

impl Default for UGSine {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGSine {
    fn type_name(&self) -> &'static str {
        "UGSine"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            vec![
                "freq".to_string(),
                "phase".to_string(),
                "min".to_string(),
                "max".to_string(),
            ]
        })
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["wave".to_string(), "trigger".to_string()])
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
        let n = wave_out.len();

        let phase_connected = phase_in.len() >= n;
        let min_connected = min_in.len() >= n;
        let max_connected = max_in.len() >= n;
        let freq_connected = freq_in.len() >= n;

        if !phase_connected && !min_connected && !max_connected {
            // common case: phase/min/max at defaults
            let phase_offset = self.default_phase_offset;
            let (min, max) = (self.default_min, self.default_max);
            if freq_connected {
                for i in 0..n {
                    self.phase += freq_in[i] * dt;
                    let crossed = self.phase >= 1.0;
                    if crossed {
                        self.phase -= 1.0;
                    }
                    let norm =
                        ((self.phase + phase_offset) * std::f32::consts::TAU).sin();
                    wave_out[i] = min + (norm + 1.0) * 0.5 * (max - min);
                    trig_out[i] = if crossed { 1.0 } else { 0.0 };
                }
            } else {
                // freq also constant: precompute phase increment
                let phase_inc = self.default_freq * dt;
                for i in 0..n {
                    self.phase += phase_inc;
                    let crossed = self.phase >= 1.0;
                    if crossed {
                        self.phase -= 1.0;
                    }
                    let norm =
                        ((self.phase + phase_offset) * std::f32::consts::TAU).sin();
                    wave_out[i] = min + (norm + 1.0) * 0.5 * (max - min);
                    trig_out[i] = if crossed { 1.0 } else { 0.0 };
                }
            }
        } else {
            // general case: at least one of phase/min/max is connected
            for i in 0..n {
                let freq = if freq_connected {
                    freq_in[i]
                } else {
                    self.default_freq
                };
                let phase_offset = if phase_connected {
                    phase_in[i]
                } else {
                    self.default_phase_offset
                };
                let min = if min_connected {
                    min_in[i]
                } else {
                    self.default_min
                };
                let max = if max_connected {
                    max_in[i]
                } else {
                    self.default_max
                };
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
}

//------------------------------------------------------------------------------

/// Waveform shape for [`UGLfo`].
#[derive(
    Debug, Clone, Copy, PartialEq, Deserialize, Serialize, strum::EnumIter, strum::Display,
)]
pub enum LfoWave {
    Sine,
    Triangle,
    Square,
}

/// Low-frequency oscillator with selectable waveform shape.
///
/// Sine is a pure sine wave. Triangle and Square are "perfect" geometries
/// (not computed from sums of sines). For both Triangle and Square the shape
/// is controlled by a duty-cycle input (`duty`) in the range `[0, 1]`.
///
/// * For Triangle: `duty = 0.5` gives equal rise and fall times;
///   `duty = 1.0` produces a rising sawtooth; `duty = 0.0` a falling sawtooth.
/// * For Square: `duty` is the fraction of the period spent high (pulse width).
///
/// All parameters (`freq`, `duty`, `min`, `max`) may be driven by signal
/// inputs at audio or control rate.
pub struct UGLfo {
    wave: LfoWave,
    mode: UnitRate,
    phase: Sample,
    default_rate: Sample,
    default_duty: Sample,
    default_min: Sample,
    default_max: Sample,
}

impl UGLfo {
    pub fn new(
        wave: LfoWave,
        rate: Sample,
        mode: UnitRate,
        duty: Sample,
        min: Sample,
        max: Sample,
    ) -> Self {
        Self {
            wave,
            mode,
            phase: 0.0,
            default_rate: rate,
            default_duty: duty,
            default_min: min,
            default_max: max,
        }
    }
}

impl UGen for UGLfo {
    fn type_name(&self) -> &'static str {
        "UGLfo"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            vec![
                "rate".to_string(),
                "duty".to_string(),
                "min".to_string(),
                "max".to_string(),
            ]
        })
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["wave".to_string()])
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "rate" => Some(self.default_rate),
            "duty" => Some(self.default_duty),
            "min" => Some(self.default_min),
            "max" => Some(self.default_max),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!(
            "wave = {:?}, rate = {}, mode = {:?}, duty = {}, min = {}, max = {}",
            self.wave,
            self.default_rate,
            self.mode,
            self.default_duty,
            self.default_min,
            self.default_max
        ))
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let rate_in = inputs.first().copied().unwrap_or(&[]);
        let duty_in = inputs.get(1).copied().unwrap_or(&[]);
        let min_in = inputs.get(2).copied().unwrap_or(&[]);
        let max_in = inputs.get(3).copied().unwrap_or(&[]);

        let wave_out = &mut outputs[0];
        let n = wave_out.len();
        let dt = 1.0 / sample_rate;

        let rate_connected = rate_in.len() >= n;
        let duty_connected = duty_in.len() >= n;
        let min_connected = min_in.len() >= n;
        let max_connected = max_in.len() >= n;

        for i in 0..n {
            let rate = if rate_connected {
                rate_in[i]
            } else {
                self.default_rate
            };
            let freq = unit_rate_to_hz(rate, self.mode, sample_rate);
            let duty = if duty_connected {
                duty_in[i]
            } else {
                self.default_duty
            }
            .clamp(0.0, 1.0);
            let min = if min_connected {
                min_in[i]
            } else {
                self.default_min
            };
            let max = if max_connected {
                max_in[i]
            } else {
                self.default_max
            };

            self.phase += freq * dt;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }

            // Compute normalised value in [0, 1]
            let norm: f32 = match self.wave {
                LfoWave::Sine => (self.phase * std::f32::consts::TAU).sin() * 0.5 + 0.5,
                LfoWave::Triangle => {
                    if duty <= 0.0 {
                        // pure falling sawtooth
                        1.0 - self.phase
                    } else if duty >= 1.0 {
                        // pure rising sawtooth
                        self.phase
                    } else if self.phase < duty {
                        self.phase / duty
                    } else {
                        (1.0 - self.phase) / (1.0 - duty)
                    }
                }
                LfoWave::Square => {
                    if self.phase < duty {
                        1.0
                    } else {
                        0.0
                    }
                }
            };

            wave_out[i] = min + norm * (max - min);
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

impl Default for UGTrigger {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGTrigger {
    fn type_name(&self) -> &'static str {
        "UGTrigger"
    }
    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["freq".to_string()])
    }
    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
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

/// Given a constant rate determined by a `rate` value and a `UnitRate`, output impulses as long as the signal input is positive.
pub struct UGClock {
    rate: Sample,
    mode: UnitRate,
    phase: Sample,
}

impl UGClock {
    pub fn new(rate: Sample, mode: UnitRate) -> Self {
        Self {
            rate,
            mode,
            phase: 1.0, // init to one to fire on first sample
        }
    }
}

impl UGen for UGClock {
    fn type_name(&self) -> &'static str {
        "UGClock"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["in".to_string()])
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "in" => Some(1.0),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("rate = {}, mode = {:?}", self.rate, self.mode))
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
        let hz = unit_rate_to_hz(self.rate, self.mode, sample_rate);
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
    fn test_mult_a() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "c1" => 3,
            "c2" => 2,
            "m1" => UGMult::new(2),
        ];
        connect_many![g,
        "c1.out" -> "m1.in1",
        "c2.out" -> "m1.in2",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("m1.out"),
            vec![6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mult_b() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "c1" => 3,
            "c2" => 2,
            "c3" => 4,
            "m1" => UGMult::new(3),
        ];
        connect_many![g,
        "c1.out" -> "m1.in1",
        "c2.out" -> "m1.in2",
        "c3.out" -> "m1.in3",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("m1.out"),
            vec![24.0, 24.0, 24.0, 24.0, 24.0, 24.0, 24.0, 24.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_pan_a() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "in" => 1,
            "pan_pos" => 0.5,
            "pan" => UGPan::new(2, 0.5),
            "rl" => UGRound::new(3, ModeRound::Round),
            "rr" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
        "in.out" -> "pan.in",
        "pan_pos.out" -> "pan.pan",
        "pan.out1" -> "rl.in",
        "pan.out2" -> "rr.in",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("rl.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
        assert_eq!(
            g.get_output_by_label("rr.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_pan_b() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "in" => 1,
            "pan_pos" => 1.5,
            "pan" => UGPan::new(4, 0.5),
            "r2" => UGRound::new(3, ModeRound::Round),
            "r3" => UGRound::new(3, ModeRound::Round),
        ];
        connect_many![g,
        "in.out" -> "pan.in",
        "pan_pos.out" -> "pan.pan",
        "pan.out2" -> "r2.in",
        "pan.out3" -> "r3.in",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("pan.out1"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("r2.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
        assert_eq!(
            g.get_output_by_label("r3.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
        assert_eq!(
            g.get_output_by_label("pan.out4"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_pan_multichannel_index_routing() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "in" => 1,
            "pan_pos_2" => 2.0,
            "pan_pos_3" => 3.0,
            "pan2" => UGPan::new(4, 0.5),
            "pan3" => UGPan::new(4, 0.5),
        ];
        connect_many![g,
        "in.out" -> "pan2.in",
        "pan_pos_2.out" -> "pan2.pan",
        "in.out" -> "pan3.in",
        "pan_pos_3.out" -> "pan3.pan",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("pan2.out1"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("pan2.out2"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("pan2.out3"),
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(
            g.get_output_by_label("pan2.out4"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );

        assert_eq!(
            g.get_output_by_label("pan3.out1"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("pan3.out2"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("pan3.out3"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("pan3.out4"),
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_floor_a() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "c1" => 2.7,
            "f1" => UGFloor::new(),
        ];
        connect_many![g,
        "c1.out" -> "f1.in",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("f1.out"),
            vec![2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_ceil_a() {
        let mut g = GenGraph::new(120.0, 8);
        register_many![g,
            "c1" => 2.3,
            "c1_neg" => -0.7,
            "cg1" => UGCeil::new(),
            "cg2" => UGCeil::new(),
        ];
        connect_many![g,
        "c1.out" -> "cg1.in",
        "c1_neg.out" -> "cg2.in",
        ];
        g.process();
        assert_eq!(
            g.get_output_by_label("cg1.out"),
            vec![3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0]
        );
        assert_eq!(
            g.get_output_by_label("cg2.out"),
            vec![-0.0, -0.0, -0.0, -0.0, -0.0, -0.0, -0.0, -0.0]
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

    //--------------------------------------------------------------------------
    #[test]
    fn test_lfo_sine_a() {
        // LFO at rate=1 Hz, sample_rate=8, buffer=8 → one full cycle
        // Phase advances by 1/8 per sample; min=0, max=1
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => c1 | Lfo(wave=Sine, rate=1) => lfo1 | Round(places=2, mode=Round) => r1 | c1 ->:rate lfo1 | lfo1 ->wave:in r1",
            8.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![0.85, 1.0, 0.85, 0.5, 0.15, 0.0, 0.15, 0.5]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_lfo_triangle_a() {
        // Triangle at rate=1, duty=0.5 → symmetric triangle, min=0, max=1
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => c1 | Lfo(wave=Triangle, rate=1) => lfo1 | Round(places=2, mode=Round) => r1 | c1 ->:rate lfo1 | lfo1 ->wave:in r1",
            8.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![0.25, 0.5, 0.75, 1.0, 0.75, 0.5, 0.25, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_lfo_triangle_sawtooth() {
        // Triangle with duty=1.0 → rising sawtooth, min=0, max=1
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => c1 | Const(value=1) => duty | Lfo(wave=Triangle, rate=1) => lfo1 | Round(places=2, mode=Round) => r1 | c1 ->:rate lfo1 | duty ->:duty lfo1 | lfo1 ->wave:in r1",
            8.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![0.12, 0.25, 0.38, 0.5, 0.62, 0.75, 0.88, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_lfo_square_a() {
        // Square at freq=1, duty=0.5 → phase < 0.5 is high, else low; min=0, max=1
        // Phase increments first: phases are [0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 0.0]
        // → 3 high, 4 low, then wraps and 1 high
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => c1 | Lfo(wave=Square, rate=1) => lfo1 | c1 ->:rate lfo1",
            8.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("lfo1.wave"),
            vec![1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mix_linear_center_pan() {
        // Single input at center pan (0.5) with level=1 → equal power in both outputs
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=0.5) => p | Const(value=1) => lv \
             | MixLinear(inputs=1, outputs=2) => mix \
             | Round(places=3, mode=Round) => r1 | Round(places=3, mode=Round) => r2 \
             | sig ->:in1 mix | p ->:pan1 mix | lv ->:level1 mix \
             | mix ->out1:in r1 | mix ->out2:in r2",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
        assert_eq!(
            g.get_output_by_label("r2.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mix_linear_hard_left() {
        // Single input panned fully left → all signal in out1, none in out2
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=0) => p | Const(value=1) => lv \
             | MixLinear(inputs=1, outputs=2) => mix \
             | sig ->:in1 mix | p ->:pan1 mix | lv ->:level1 mix",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("mix.out1"),
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(
            g.get_output_by_label("mix.out2"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mix_linear_hard_right() {
        // Single input panned fully right → all signal in out2, none in out1
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=1) => p | Const(value=1) => lv \
             | MixLinear(inputs=1, outputs=2) => mix \
             | sig ->:in1 mix | p ->:pan1 mix | lv ->:level1 mix",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("mix.out1"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
        assert_eq!(
            g.get_output_by_label("mix.out2"),
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mix_linear_two_inputs_summed() {
        // Two inputs panned to opposite extremes; outputs should each receive one signal
        let mut g = crate::graph_from_chain_expression(
            "Const(value=0.5) => s1 | Const(value=0.5) => s2 \
             | Const(value=0) => pl | Const(value=1) => pr | Const(value=1) => lv \
             | MixLinear(inputs=2, outputs=2) => mix \
             | s1 ->:in1 mix | pl ->:pan1 mix | lv ->:level1 mix \
             | s2 ->:in2 mix | pr ->:pan2 mix | lv ->:level2 mix",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("mix.out1"),
            vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]
        );
        assert_eq!(
            g.get_output_by_label("mix.out2"),
            vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mix_linear_level_log_scaling() {
        // level=0.5 → gain = 1000^(0.5-1) = 1000^(-0.5) ≈ 0.03162; panned hard left
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=0) => p | Const(value=0.5) => lv \
             | MixLinear(inputs=1, outputs=2) => mix \
             | Round(places=3, mode=Round) => r1 \
             | sig ->:in1 mix | p ->:pan1 mix | lv ->:level1 mix \
             | mix ->out1:in r1",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        // 1000^(-0.5) = 10^(-1.5) ≈ 0.03162 → rounds to 0.032
        assert_eq!(
            g.get_output_by_label("r1.out"),
            vec![0.032, 0.032, 0.032, 0.032, 0.032, 0.032, 0.032, 0.032]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_mix_linear_level_zero_is_silence() {
        // level=0 → gain = 0 → silence regardless of signal
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=0) => p | Const(value=0) => lv \
             | MixLinear(inputs=1, outputs=2) => mix \
             | sig ->:in1 mix | p ->:pan1 mix | lv ->:level1 mix",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("mix.out1"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_fade_unity_gain() {
        // level=1 → gain = 1000^0 = 1.0 → signal unchanged
        let mut g = crate::graph_from_chain_expression(
            "Const(value=0.5) => sig | Const(value=1) => lv \
             | Fade(channels=1) => fd \
             | sig ->:in1 fd | lv ->:level fd",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("fd.out1"),
            vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_fade_zero_is_silence() {
        // level=0 → gain = 0 → silence
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=0) => lv \
             | Fade(channels=1) => fd \
             | sig ->:in1 fd | lv ->:level fd",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("fd.out1"),
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_fade_log_scaling() {
        // level=0.5 → gain = 1000^(-0.5) ≈ 0.03162
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => sig | Const(value=0.5) => lv \
             | Fade(channels=1) => fd \
             | Round(places=3, mode=Round) => r \
             | sig ->:in1 fd | lv ->:level fd \
             | fd ->out1:in r",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        // 1000^(-0.5) = 10^(-1.5) ≈ 0.03162 → rounds to 0.032
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![0.032, 0.032, 0.032, 0.032, 0.032, 0.032, 0.032, 0.032]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_fade_two_channels_same_gain() {
        // Two channels scaled by the same level; channels remain independent
        let mut g = crate::graph_from_chain_expression(
            "Const(value=1) => s1 | Const(value=0.5) => s2 | Const(value=1) => lv \
             | Fade(channels=2) => fd \
             | s1 ->:in1 fd | s2 ->:in2 fd | lv ->:level fd",
            120.0,
            8,
        )
        .unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("fd.out1"),
            vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]
        );
        assert_eq!(
            g.get_output_by_label("fd.out2"),
            vec![0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]
        );
    }
}
