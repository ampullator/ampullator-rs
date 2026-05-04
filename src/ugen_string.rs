use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::UGen;
use crate::util::Sample;

/// Karplus-Strong string synthesis UGen.
///
/// Produces a plucked-string timbre by initialising a delay line with white
/// noise on each trigger event and then feeding it back through a one-pole
/// lowpass (averaging) filter.  The delay-line length sets the fundamental
/// pitch; the damping coefficient controls how quickly the sound decays.
///
/// Reference: <https://en.wikipedia.org/wiki/Karplus%E2%80%93Strong_string_synthesis>
///
/// Inputs:
///   0 trigger – Rising edge (≤ 0.5 → > 0.5) re-initialises the delay line and
///               starts a new note.  Default: 0.0
///   1 freq    – Fundamental frequency in Hz; sampled at the moment of each
///               trigger event to set the delay-line length.  Default: 440.0
///   2 damping – Feedback coefficient in [0.0, 1.0]; applied to the one-pole
///               averaging filter on every sample.  Values close to 1.0 produce
///               long, slowly-decaying tones; lower values cause faster decay.
///               Default: 0.996
///
/// Outputs:
///   0 out – Audio output signal.
pub struct UGString {
    /// Delay-line ring buffer. Grown lazily as needed.
    buffer: Vec<Sample>,
    /// Active delay-line length in samples (= round(sample_rate / freq)).
    delay_len: usize,
    /// Current read/write head position within the ring buffer.
    read_pos: usize,
    /// Random-number generator used to seed the delay line on each trigger.
    rng: StdRng,
    /// Optional seed stored for `describe_config`.
    seed: Option<u64>,
    /// Default frequency (Hz); also reported by `default_input`.
    default_freq: f32,
    /// Default damping coefficient; also reported by `default_input`.
    default_damping: f32,
    /// Most-recently-seen trigger value, used to detect rising edges.
    prev_trigger: Sample,
}

impl UGString {
    /// Create a new `UGString`.
    ///
    /// * `freq`    – Fundamental frequency in Hz (e.g. `440.0`).
    /// * `damping` – Feedback decay coefficient in `[0.0, 1.0]`.  Values close
    ///               to `1.0` produce long, slowly-decaying tones.  The classic
    ///               Karplus-Strong algorithm approximates `0.996` at 44 100 Hz.
    /// * `seed`    – Optional RNG seed for reproducible output.
    pub fn new(freq: f32, damping: f32, seed: Option<u64>) -> Self {
        let actual_seed = seed.unwrap_or_else(|| rand::rng().random());
        Self {
            buffer: Vec::new(),
            delay_len: 0,
            read_pos: 0,
            rng: StdRng::seed_from_u64(actual_seed),
            seed,
            default_freq: freq,
            default_damping: damping,
            prev_trigger: 0.0,
        }
    }
}

impl UGen for UGString {
    fn type_name(&self) -> &'static str {
        "UGString"
    }

    fn input_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| {
            vec![
                "trigger".to_string(),
                "freq".to_string(),
                "damping".to_string(),
            ]
        })
    }

    fn output_names(&self) -> &[String] {
        static NAMES: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
        NAMES.get_or_init(|| vec!["out".to_string()])
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "trigger" => Some(0.0),
            "freq" => Some(self.default_freq),
            "damping" => Some(self.default_damping),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        let seed_str = self
            .seed
            .map_or("none".to_string(), |s| s.to_string());
        Some(format!(
            "freq = {}, damping = {}, seed = {seed_str}",
            self.default_freq, self.default_damping
        ))
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let trigger = inputs.first().copied().unwrap_or(&[]);
        let freq_in = inputs.get(1).copied().unwrap_or(&[]);
        let damping_in = inputs.get(2).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        for i in 0..out.len() {
            let trig = trigger.get(i).copied().unwrap_or(0.0);
            let freq = freq_in
                .get(i)
                .copied()
                .unwrap_or(self.default_freq)
                .max(1.0);
            let damping = damping_in
                .get(i)
                .copied()
                .unwrap_or(self.default_damping)
                .clamp(0.0, 1.0);

            // Detect rising edge of the trigger signal.
            let triggered = trig > 0.5 && self.prev_trigger <= 0.5;
            self.prev_trigger = trig;

            if triggered {
                // Re-initialise the delay line with white noise at the
                // frequency sampled from the `freq` input at trigger time.
                let new_len = ((sample_rate / freq).round() as usize).max(1);
                if self.buffer.len() < new_len {
                    self.buffer.resize(new_len, 0.0);
                }
                self.delay_len = new_len;
                for s in self.buffer[..new_len].iter_mut() {
                    *s = self.rng.random_range(-1.0_f32..=1.0_f32);
                }
                self.read_pos = 0;
            }

            // Output silence until the first trigger has been received.
            if self.delay_len == 0 {
                out[i] = 0.0;
                continue;
            }

            // Output the current sample, then apply the one-pole averaging
            // (lowpass) filter: new_sample = damping * 0.5 * (current + next).
            let current = self.buffer[self.read_pos];
            let next_pos = (self.read_pos + 1) % self.delay_len;
            self.buffer[self.read_pos] =
                damping * 0.5 * (current + self.buffer[next_pos]);
            self.read_pos = next_pos;
            out[i] = current;
        }
    }
}

//------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that no output is produced before the first trigger.
    #[test]
    fn test_silence_before_trigger() {
        let mut ug = UGString::new(440.0, 0.996, Some(0));
        let trigger = vec![0.0_f32; 8];
        let inputs: Vec<&[f32]> = vec![&trigger];
        let mut buf = vec![0.0_f32; 8];
        let mut outputs: Vec<&mut [f32]> = vec![&mut buf];
        ug.process(&inputs, &mut outputs, 44100.0, 0);
        assert!(
            outputs[0].iter().all(|&s| s == 0.0),
            "expected silence before trigger"
        );
    }

    /// Verify that a trigger causes non-zero output (with a non-zero-seed delay line).
    #[test]
    fn test_output_after_trigger() {
        let mut ug = UGString::new(440.0, 0.996, Some(42));
        // First sample is the trigger, the rest are silent.
        let trigger = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let inputs: Vec<&[f32]> = vec![&trigger];
        let mut buf = vec![0.0_f32; 8];
        let mut outputs: Vec<&mut [f32]> = vec![&mut buf];
        ug.process(&inputs, &mut outputs, 44100.0, 0);
        // At least one sample after the trigger should be non-zero.
        assert!(
            outputs[0].iter().any(|&s| s != 0.0),
            "expected non-zero output after trigger"
        );
    }

    /// Verify deterministic output with a fixed seed.
    #[test]
    fn test_deterministic_with_seed() {
        let trigger = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let run = |seed: u64| {
            let mut ug = UGString::new(440.0, 0.996, Some(seed));
            let inputs: Vec<&[f32]> = vec![&trigger];
            let mut buf = vec![0.0_f32; 8];
            let mut outputs: Vec<&mut [f32]> = vec![&mut buf];
            ug.process(&inputs, &mut outputs, 44100.0, 0);
            buf
        };

        assert_eq!(run(99), run(99), "same seed must give identical output");
        // Different seeds should (almost certainly) differ.
        assert_ne!(run(1), run(2), "different seeds should give different output");
    }
}
