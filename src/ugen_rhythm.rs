use crate::ModeSelect;
use crate::UGSelect;
use crate::UGen;
use crate::util::Sample;





pub struct UGPulseSelect {
    pulse_counter: usize,
    required_pulses: usize,
    duration_select: UGSelect,
    level_select: UGSelect,
}

impl UGPulseSelect {
    pub fn new(
        duration_values: Vec<Sample>,
        duration_mode: ModeSelect,
        level_values: Vec<Sample>,
        level_mode: ModeSelect,
        seed: Option<u64>,
    ) -> Self {
        Self {
            pulse_counter: 0,
            required_pulses: 1,
            duration_select: UGSelect::new(duration_values, duration_mode, seed),
            level_select: UGSelect::new(level_values, level_mode, seed),
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
        let clock = inputs.get(0).copied().unwrap_or(&[]);
        let step = inputs.get(1).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let triggered = clock.get(i).copied().unwrap_or(0.0) > 0.5;

            if triggered {
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
