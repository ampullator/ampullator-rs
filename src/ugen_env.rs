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
    // triggered_last: bool,
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
            // triggered_last: false,
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
            let triggered_now = clock.get(i).copied().unwrap_or(0.0) > 0.5;
            // let triggered_now = clock_now && !self.triggered_last;
            // self.triggered_last = clock_now;

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
    stage_total: i32,
    stage_remain: i32,
    curve: Sample,
}

impl UGEnvAR {
    pub fn new() -> Self {
        Self {
            current: 0.0,
            phase: EnvPhase::Idle,
            start: 0.0,
            target: 1.0,
            stage_total: 0,
            stage_remain: 0,
            curve: 1.0,
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
            let trigger_now = trigger.get(i).copied().unwrap_or(0.0) > 0.5;

            if trigger_now {
                self.phase = EnvPhase::Attack;
                self.start = self.current;
                self.target = 1.0;
                self.stage_total =
                    att_dur.get(i).copied().unwrap_or(1.0).max(1.0).round() as i32;
                self.stage_remain = self.stage_total;
                self.curve = att_curve.get(i).copied().unwrap_or(1.0).max(0.001);
            }

            if self.stage_remain >= 0 {
                // get an f32 percent completion
                let progress = 1.0 - (self.stage_remain as f32 / self.stage_total as f32);

                let shaped = if (self.curve - 1.0).abs() < 1e-6 {
                    progress
                } else {
                    1.0 - (-self.curve * progress).exp()
                };

                // scale availalbe range by shaped
                self.current = self.start + (self.target - self.start) * shaped;
                self.stage_remain -= 1;

                if self.stage_remain < 0 {
                    // given matched phase, determine next
                    match self.phase {
                        EnvPhase::Attack => {
                            self.phase = EnvPhase::Release;
                            self.start = self.current;
                            self.target = 0.0;
                            self.stage_total =
                                rel_dur.get(i).copied().unwrap_or(1.0).max(1.0).round()
                                    as i32;
                            self.stage_remain = self.stage_total;
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
    use crate::ModeRound;
    use crate::Recorder;
    use crate::UGClock;
    use crate::UGRound;
    use crate::UGSum;
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

    #[test]
    fn test_env_break_point_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(1.0, UnitRate::Samples),
            "step" => 1,
            "env" => UGEnvBreakPoint::new(
                vec![2., 4., 3., 8.], // dur
                ModeSelect::Cycle,
                vec![1., 2., 3., 4., 5., 6., 7., 8.], // value
                ModeSelect::Cycle,
                Some(42),
            ),
        ];

        connect_many![g,
            "clock.out" -> "env.clock",
            "step.out" -> "env.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("env.out"),
            vec![
                1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 4.0, 4.0, 4.0,
                4.0, 4.0, 4.0, 5.0, 5.0, 6.0, 6.0, 6.0, 6.0, 7.0, 7.0, 7.0, 8.0, 8.0,
                8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0,
                3.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 5.0, 5.0, 6.0, 6.0, 6.0,
                6.0, 7.0, 7.0, 7.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 1.0, 1.0,
                2.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0, 4.0,
                4.0, 5.0, 5.0, 6.0, 6.0, 6.0, 6.0, 7.0, 7.0, 7.0, 8.0, 8.0, 8.0, 8.0,
                8.0, 8.0
            ]
        );
    }

    #[test]
    fn test_env_break_point_c() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(1.0, UnitRate::Samples),
            "step" => 1,
            "env" => UGEnvBreakPoint::new(
                vec![2., 4., 8.], // dur
                ModeSelect::Cycle,
                vec![1., 2., 3., 4., 5., 6., 7., 8.], // value
                ModeSelect::Walk,
                Some(42),
            ),
        ];

        connect_many![g,
            "clock.out" -> "env.clock",
            "step.out" -> "env.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);
        r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("env.out"),
            vec![
                7.0, 7.0, 6.0, 6.0, 6.0, 6.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0,
                6.0, 6.0, 7.0, 7.0, 7.0, 7.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0,
                7.0, 7.0, 6.0, 6.0, 6.0, 6.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0,
                8.0, 8.0, 7.0, 7.0, 7.0, 7.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0, 6.0,
                7.0, 7.0, 8.0, 8.0, 8.0, 8.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0,
                8.0, 8.0, 1.0, 1.0, 1.0, 1.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0, 8.0,
                1.0, 1.0, 8.0, 8.0, 8.0, 8.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0,
                8.0, 8.0
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
                0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 0.875, 0.75, 0.625, 0.5, 0.375, 0.25,
                0.125, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0,
                0.875, 0.75, 0.625, 0.5, 0.375, 0.25, 0.125, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0
            ]
        );
    }

    #[test]
    fn test_env_ar_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(30.0, UnitRate::Samples),
            "env" => UGEnvAR::new(),
            "a" => 8,
            "r" => 17,
            "a-curve" => 1,
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
            "clock.out" -> "env.trigger",
            "a.out" -> "env.attack_dur",
            "a-curve.out" -> "env.attack_curve",
            "r.out" -> "env.release_dur",
            "env.out" -> "round.in"
        ];

        let r1 = Recorder::from_samples(g, None, 100);
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("round.out"),
            vec![
                0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.0, 0.9412,
                0.8824, 0.8235, 0.7647, 0.7059, 0.6471, 0.5882, 0.5294, 0.4706, 0.4118,
                0.3529, 0.2941, 0.2353, 0.1765, 0.1176, 0.0588, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.0, 0.9412, 0.8824,
                0.8235, 0.7647, 0.7059, 0.6471, 0.5882, 0.5294, 0.4706, 0.4118, 0.3529,
                0.2941, 0.2353, 0.1765, 0.1176, 0.0588, 0.0, 0.0, 0.0, 0.0, 0.0, 0.125,
                0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.0, 0.9412, 0.8824, 0.8235,
                0.7647, 0.7059, 0.6471, 0.5882, 0.5294, 0.4706, 0.4118, 0.3529, 0.2941,
                0.2353, 0.1765, 0.1176, 0.0588, 0.0, 0.0, 0.0, 0.0, 0.0, 0.125, 0.25,
                0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.0
            ]
        );
    }

    #[test]
    fn test_env_ar_c() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock1" => UGClock::new(30.0, UnitRate::Samples),
            "clock2" => UGClock::new(47.0, UnitRate::Samples),
            "sum" => UGSum::new(2),
            "env" => UGEnvAR::new(),
            "a" => 8,
            "r" => 17,
            "a-curve" => 1,
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
            "clock1.out" -> "sum.in1",
            "clock2.out" -> "sum.in2",
            "sum.out" -> "env.trigger",
            "a.out" -> "env.attack_dur",
            "a-curve.out" -> "env.attack_curve",
            "r.out" -> "env.release_dur",
            "env.out" -> "round.in"
        ];

        let r1 = Recorder::from_samples(g, None, 100);
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("round.out"),
            vec![
                0.0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.0, 0.9412,
                0.8824, 0.8235, 0.7647, 0.7059, 0.6471, 0.5882, 0.5294, 0.4706, 0.4118,
                0.3529, 0.2941, 0.2353, 0.1765, 0.1176, 0.0588, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.0, 0.9412, 0.8824,
                0.8235, 0.7647, 0.7059, 0.6471, 0.5882, 0.5294, 0.5294, 0.5882, 0.6471,
                0.7059, 0.7647, 0.8235, 0.8824, 0.9412, 1.0, 1.0, 0.9412, 0.8824, 0.8824,
                0.8971, 0.9118, 0.9265, 0.9412, 0.9559, 0.9706, 0.9853, 1.0, 1.0, 0.9412,
                0.8824, 0.8235, 0.7647, 0.7059, 0.6471, 0.5882, 0.5294, 0.4706, 0.4118,
                0.3529, 0.2941, 0.2353, 0.1765, 0.1176, 0.0588, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.125, 0.25, 0.375, 0.5, 0.625, 0.625, 0.6719, 0.7188, 0.7656
            ]
        );
    }
}
