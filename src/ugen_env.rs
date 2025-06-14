use crate::ModeSelect;
use crate::UGSelect;
use crate::UGen;
use crate::util::Sample;

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

/// An attack-release envelope, with signal controllable attack and release times, as well as attack and release curves.
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
                // immediately switch if we've just hit the final sample
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

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::ModeRound;
    use crate::Recorder;
    use crate::UGClock;
    use crate::UGRound;
    use crate::connect_many;
    use crate::register_many;
    use crate::util::UnitRate;

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

    #[test]
    fn test_env_ar_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(16.0, UnitRate::Samples),
            "env" => UGEnvAR::new(),
            "a" => 4,
            "r" => 8,
            "a-curve" => 0.5,
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
            "clock.out" -> "env.trigger",
            "a.out" -> "env.attack_dur",
            "a-curve.out" -> "env.attack_curve",
            "r.out" -> "env.release_dur",
            "env.out" -> "round.in"
        ];

        // let output_labels = Some(vec!["round.out".to_string()]);
        let r1 = Recorder::from_samples(g, None, 120);
        r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();
    }
}
