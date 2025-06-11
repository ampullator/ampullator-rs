use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng, rngs::StdRng};

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
        let input = inputs.get(0).copied().unwrap_or(&[]);
        let out = &mut outputs[0];
        for i in 0..out.len() {
            let x = input.get(i).copied().unwrap_or(0.0);
            out[i] = unit_rate_to_hz(x, self.mode, sample_rate)
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

//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum ModeSelect {
    Cycle,
    Random,
    Shuffle,
    Walk,
}

#[derive(Clone)]
pub struct UGSelect {
    values: Vec<Sample>,
    mode: ModeSelect,
    index: usize,
    shuffle_remaining: Vec<usize>,
    rng: StdRng,
}

impl UGSelect {
    pub fn new(values: Vec<Sample>, mode: ModeSelect, seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_rng(&mut rand::rng()),
        };
        let len = values.len().max(1);
        UGSelect {
            values,
            mode,
            index: len - 1, // not optimal
            shuffle_remaining: Vec::new(),
            rng,
        }
    }

    pub fn select_next(
        &mut self,
        step: Sample,
        sample_rate: f32,
        time_sample: usize,
    ) -> Sample {
        let trigger = [1.0];
        let step = [step];
        let mut out = [0.0];
        let inputs = [&trigger[..], &step[..]];
        let mut outputs = [&mut out[..]];

        self.process(&inputs, &mut outputs, sample_rate, time_sample);
        out[0]
    }
}

impl UGen for UGSelect {
    fn type_name(&self) -> &'static str {
        "UGSelect"
    }

    fn input_names(&self) -> &[&'static str] {
        &["trigger", "step"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "trigger" => Some(0.0),
            "step" => Some(1.0),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("values = {:?}, mode = {:?}", self.values, self.mode).to_lowercase())
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let trigger = inputs.get(0).copied().unwrap_or(&[]);
        let step = inputs.get(1).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        let n = self.values.len();
        if n == 0 {
            for o in out.iter_mut() {
                *o = 0.0;
            }
            return;
        }

        for i in 0..out.len() {
            if trigger.get(i).copied().unwrap_or(0.0) == 1.0 {
                let step_size =
                    step.get(i).copied().unwrap_or(1.0).round().max(1.0) as usize;

                match self.mode {
                    ModeSelect::Cycle => {
                        self.index = (self.index + step_size) % n;
                    }
                    ModeSelect::Random => {
                        self.index = self.rng.random_range(0..n);
                    }
                    ModeSelect::Shuffle => {
                        for _ in 0..step_size {
                            if self.shuffle_remaining.is_empty() {
                                self.shuffle_remaining = (0..n).collect();
                                self.shuffle_remaining.shuffle(&mut self.rng);
                            }
                            self.index = self.shuffle_remaining.pop().unwrap();
                        }
                    }
                    ModeSelect::Walk => {
                        let direction = if self.rng.random_bool(0.5) { 1 } else { -1 };
                        let step_signed = step_size as isize * direction as isize;
                        let new_index = ((self.index as isize + step_signed)
                            .rem_euclid(n as isize))
                            as usize;
                        self.index = new_index;
                    }
                }
            }
            out[i] = self.values[self.index];
        }
    }
}
//------------------------------------------------------------------------------
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
            phase: 0.0,
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
        inputs: &[&[Sample]], // optional on/off input
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let enabled = inputs.get(0).copied().unwrap_or(&[]);
        let out = &mut outputs[0];
        let hz = unit_rate_to_hz(self.value, self.mode, sample_rate);

        out[0] = if enabled.get(0).copied().unwrap_or(1.0) > 0.5 {
            1.0
        } else {
            0.0
        };

        for i in 1..out.len() {
            let on = enabled.get(i).copied().unwrap_or(1.0) > 0.5;
            if !on {
                out[i] = 0.0;
                continue;
            }
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
// UGEnvBreakPoint

#[derive(Clone)]
pub struct UGEnvBreakPoint {
    current: Sample,
    pulse_counter: usize,
    required_pulses: usize,

    duration_select: UGSelect,
    level_select: UGSelect,

    triggered_last: bool,
}

impl UGEnvBreakPoint {
    pub fn new(
        duration_values: Vec<Sample>,
        duration_mode: ModeSelect,
        level_values: Vec<Sample>,
        level_mode: ModeSelect,
        seed: Option<u64>,
    ) -> Self {
        Self {
            current: 0.0,
            pulse_counter: 0,
            required_pulses: 1,
            duration_select: UGSelect::new(duration_values, duration_mode, seed),
            level_select: UGSelect::new(level_values, level_mode, seed),
            triggered_last: false,
        }
    }
}

impl UGen for UGEnvBreakPoint {
    fn type_name(&self) -> &'static str {
        "UGEnvBreakPoint"
    }

    fn input_names(&self) -> &[&'static str] {
        &["clock", "step"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "clock" => Some(0.0),
            "step" => Some(1.0),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        Some("stepped".into())
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        time_sample: usize,
    ) {
        let clock = inputs.get(0).copied().unwrap_or(&[]);
        let step = inputs.get(1).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let clock_now = clock.get(i).copied().unwrap_or(0.0) > 0.5;
            let triggered_now = clock_now && !self.triggered_last;
            self.triggered_last = clock_now;

            if triggered_now {
                let step_size = step.get(i).copied().unwrap_or(1.0).max(1.0).round();

                self.pulse_counter += 1;

                if self.pulse_counter >= self.required_pulses {
                    self.pulse_counter = 0;

                    self.required_pulses = self
                        .duration_select
                        .select_next(step_size, sample_rate, time_sample)
                        .max(1.0)
                        .round() as usize;

                    self.current = self.level_select.select_next(
                        step_size,
                        sample_rate,
                        time_sample,
                    );
                }
            }

            out[i] = self.current;
        }
    }
}

//------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum EnvPhase {
    Idle,
    Attack,
    Release,
}

#[derive(Clone)]
pub struct UGEnvAR {
    current: Sample,
    phase: EnvPhase,
    start: Sample,
    target: Sample,
    total_samples: usize,
    remaining_samples: usize,
    curve: Sample,
    triggered_last: bool,
}

impl UGEnvAR {
    pub fn new() -> Self {
        Self {
            current: 0.0,
            phase: EnvPhase::Idle,
            start: 0.0,
            target: 1.0,
            total_samples: 0,
            remaining_samples: 0,
            curve: 1.0,
            triggered_last: false,
        }
    }
}

impl UGen for UGEnvAR {
    fn type_name(&self) -> &'static str {
        "UGEnvAR"
    }

    fn input_names(&self) -> &[&'static str] {
        &[
            "trigger",
            "attack_dur",
            "release_dur",
            "attack_curve",
            "release_curve",
        ]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "trigger" => Some(0.0),
            "attack_dur" | "release_dur" => Some(1.0),
            "attack_curve" | "release_curve" => Some(1.0),
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
        let trigger = inputs.get(0).copied().unwrap_or(&[]);
        let att_dur = inputs.get(1).copied().unwrap_or(&[]);
        let rel_dur = inputs.get(2).copied().unwrap_or(&[]);
        let att_curve = inputs.get(3).copied().unwrap_or(&[]);
        let rel_curve = inputs.get(4).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let is_triggered = trigger.get(i).copied().unwrap_or(0.0) > 0.5;
            let trigger_now = is_triggered && !self.triggered_last;
            self.triggered_last = is_triggered;

            if trigger_now {
                self.phase = EnvPhase::Attack;
                self.start = self.current;
                self.target = 1.0;
                self.total_samples =
                    att_dur.get(i).copied().unwrap_or(1.0).max(1.0).round() as usize;
                self.remaining_samples = self.total_samples;
                self.curve = att_curve.get(i).copied().unwrap_or(1.0).max(0.001);
            }

            if self.remaining_samples > 0 {
                // ⬅ use remaining_samples - 1 to make progress hit 1.0 earlier
                let progress = if self.remaining_samples > 1 {
                    1.0 - ((self.remaining_samples - 1) as f32
                        / self.total_samples as f32)
                } else {
                    1.0
                };

                let shaped = if (self.curve - 1.0).abs() < 1e-6 {
                    progress
                } else {
                    1.0 - (-self.curve * progress).exp()
                };

                self.current = self.start + (self.target - self.start) * shaped;
                self.remaining_samples -= 1;
                // 💡 immediately switch if we've just hit the final sample
                if self.remaining_samples == 0 {
                    match self.phase {
                        EnvPhase::Attack => {
                            self.phase = EnvPhase::Release;
                            self.start = self.current;
                            self.target = 0.0;
                            self.total_samples =
                                rel_dur.get(i).copied().unwrap_or(1.0).max(1.0).round()
                                    as usize;
                            self.remaining_samples = self.total_samples;
                            self.curve =
                                rel_curve.get(i).copied().unwrap_or(1.0).max(0.001);
                        }
                        EnvPhase::Release => {
                            self.phase = EnvPhase::Idle;
                            self.current = 0.0;
                        }
                        EnvPhase::Idle => {}
                    }
                }
            }

            out[i] = self.current;
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

    // use crate::plot_graph_to_image;

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
    fn test_select_a() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "s1" => UGSelect::new(
                vec![3.0, 10.0, 20.0, 50.0, 999.0],
                ModeSelect::Cycle,
                Some(42)),
            "c1" => 1,
        ];
        g.connect("c1.out", "s1.trigger");
        g.process();

        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![3.0, 10.0, 20.0, 50.0, 999.0, 3.0, 10.0, 20.0]
        )
    }

    #[test]
    fn test_select_b() {
        let mut g = GenGraph::new(8.0, 16);
        register_many![g,
            "s1" => UGSelect::new(vec![3.0, 10.0, 20.0, 50.0], ModeSelect::Walk, Some(42)),
            "c1" => 1.0,
        ];
        g.connect("c1.out", "s1.trigger");
        g.process();

        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![
                20.0, 10.0, 3.0, 10.0, 20.0, 50.0, 20.0, 10.0, 20.0, 50.0, 20.0, 10.0,
                20.0, 50.0, 20.0, 50.0
            ]
        )
    }

    #[test]
    fn test_select_c() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![
            g,
            "s1" => UGSelect::new(
                vec![3.0, 10.0, 20.0, 50.0, 99.0],
                ModeSelect::Shuffle,
                Some(42)),
            "c1" => 1,
        ];
        connect_many![g, "c1.out" -> "s1.trigger"];
        g.process();

        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![
                10.0, 20.0, 50.0, 3.0, 99.0, 20.0, 99.0, 10.0, 50.0, 3.0, 50.0, 20.0,
                10.0, 3.0, 99.0, 10.0, 50.0, 3.0, 99.0, 20.0
            ]
        );
        assert_eq!(
            format!("\n{}", g.describe()),
            r#"
c1 <UGConst {value = 1.000}>
→ out ≊ 1.000

s1 <UGSelect {values = [3.0, 10.0, 20.0, 50.0, 99.0], mode = shuffle}>
trigger ← c1.out
step ←= 1.000
→ out ≊ 20.000
"#
        );

        // plot_graph_to_image(&g, "/tmp/ampullator.png").unwrap();
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
    fn test_env_break_point_a() {
        let mut g = GenGraph::new(8.0, 40);
        register_many![g,
            "clock" => UGClock::new(2.0, UnitRate::Samples),
            "env_st" => UGEnvBreakPoint::new(
                vec![2.0, 4.0, 3.0, 2.0], ModeSelect::Cycle,
                vec![1.0, 0.2, 0.8, 0.5], ModeSelect::Cycle,
                Some(42),
            ),
            "r" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
        "clock.out" -> "env_st.clock",
        "env_st.out" -> "r.in",
        ];

        g.process();

        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.0, 1.0, 1.0, 1.0, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.8, 0.8,
                0.8, 0.8, 0.8, 0.8, 0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 0.2, 0.2,
                0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.8, 0.8, 0.8, 0.8, 0.8, 0.8
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_env_ar_a() {
        let mut g = GenGraph::new(8.0, 40);
        register_many![g,
            "clock" => UGClock::new(20.0, UnitRate::Samples),
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

        assert_eq!(
            g.get_output_by_label("round.out"),
            vec![
                0.25, 0.5, 0.75, 1.0, 0.875, 0.75, 0.625, 0.5, 0.375, 0.25, 0.125, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 0.875,
                0.75, 0.625, 0.5, 0.375, 0.25, 0.125, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0
            ]
        );
    }
}
