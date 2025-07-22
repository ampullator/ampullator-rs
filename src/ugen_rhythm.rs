use crate::ModeSelect;
use crate::UGSelect;
use crate::UGen;
use crate::util::Sample;

pub struct UGPulseSelect {
    pulse_count: usize,
    pulse_target: usize,
    duration_select: UGSelect,
}

impl UGPulseSelect {
    pub fn new(
        duration_values: Vec<Sample>,
        duration_mode: ModeSelect,
        seed: Option<u64>,
    ) -> Self {
        Self {
            pulse_count: 0,
            pulse_target: 1,
            duration_select: UGSelect::new(duration_values, duration_mode, seed),
        }
    }
}

impl UGen for UGPulseSelect {
    fn type_name(&self) -> &'static str {
        "UGPulseSelect"
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
        let clock = inputs.first().copied().unwrap_or(&[]);
        let step = inputs.get(1).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let triggered = clock.get(i).copied().unwrap_or(0.0) > 0.5;

            if triggered {
                if self.pulse_count == 0 {
                    out[i] = 1.0;

                    let step_size = step.get(i).copied().unwrap_or(1.0).max(1.0).round();
                    self.pulse_target = self
                        .duration_select
                        .select_next(step_size, sample_rate, time_sample)
                        .max(1.0)
                        .round() as usize;
                } else {
                    out[i] = 0.0;
                }

                self.pulse_count += 1;
                if self.pulse_count >= self.pulse_target {
                    self.pulse_count = 0;
                }
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
    use crate::Recorder;
    use crate::UGClock;
    use crate::UnitRate;
    use crate::connect_many;
    use crate::register_many;

    #[test]
    fn test_pulse_select_a() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(1.0, UnitRate::Samples),
            "step" => 1,
            "pulse" => UGPulseSelect::new(
                vec![3., 1., 4., 2.],
                ModeSelect::Cycle,
                Some(42),
            ),
        ];

        connect_many![g,
            "clock.out" -> "pulse.clock",
            "step.out" -> "pulse.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("pulse.out"),
            vec![
                1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0,
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0,
                1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0,
                0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0,
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0,
                1.0, 0.0
            ]
        );
    }

    #[test]
    fn test_pulse_select_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(2.0, UnitRate::Samples),
            "step" => 1,
            "pulse" => UGPulseSelect::new(
                vec![3., 1., 4., 2.],
                ModeSelect::Cycle,
                Some(42),
            ),
        ];

        connect_many![g,
            "clock.out" -> "pulse.clock",
            "step.out" -> "pulse.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("pulse.out"),
            vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                0.0, 0.0
            ]
        );
    }

    #[test]
    fn test_pulse_select_c() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(2.0, UnitRate::Samples),
            "step" => 1,
            "pulse" => UGPulseSelect::new(
                vec![1., 8., 2., 1.],
                ModeSelect::Shuffle,
                Some(42),
            ),
        ];

        connect_many![g,
            "clock.out" -> "pulse.clock",
            "step.out" -> "pulse.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("pulse.out"),
            vec![
                1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                0.0, 0.0
            ]
        );
    }
}
