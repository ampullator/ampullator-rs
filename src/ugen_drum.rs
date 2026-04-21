use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::UGen;
use crate::util::Sample;

/// ln(1000) ≈ 6.9078; used so that `exp(-LN_1000 / decay_samples)` gives a
/// coefficient that reaches -60 dB (0.001) after exactly `decay_samples` steps.
const LN_1000: f32 = 6.907_755;

//------------------------------------------------------------------------------
// UGSnareDrum

/// An analog-style snare drum, modeled after circuits found in the Roland TR-808 and TR-909.
///
/// The drum combines a tuned tonal body (sine oscillator with exponential pitch sweep and
/// amplitude decay) with a noise component (high-pass filtered white noise with exponential
/// amplitude decay). Both components use exponential envelopes and the output passes through
/// soft saturation for analog warmth.
///
/// All parameters are signal inputs, enabling dynamic per-hit modulation.
///
/// Inputs:
///   0 gate          - Trigger input; a rising edge (≤0.5 → >0.5) fires the drum.
///   1 tune          - Fundamental frequency of the tonal body in Hz. Default: 180.0
///   2 tone          - Level of the tonal body [0..1]. Default: 0.7
///   3 snappy        - Level of the noise component (snare wire rattle) [0..1]. Default: 0.9
///   4 tone_decay    - Decay time of the tonal body in samples. Default: 3000.0
///   5 snappy_decay  - Decay time of the noise component in samples. Default: 5000.0
///   6 noise_filter  - High-pass cutoff frequency (Hz) for noise coloring. Default: 4000.0
///   7 pitch_sweep   - Pitch sweep multiplier; starting freq = tune * pitch_sweep. Default: 1.5
///
/// Outputs:
///   0 out - Mixed, soft-saturated snare output in approximately [-1..1].
pub struct UGSnareDrum {
    // Tonal oscillator internal phase (0..1)
    tone_phase: Sample,
    // Exponential amplitude envelope for the tonal body (1.0 at trigger, decays to 0)
    tone_env: Sample,
    // Exponential amplitude envelope for the noise component (1.0 at trigger, decays to 0)
    snappy_env: Sample,
    // Exponential pitch sweep envelope (1.0 at trigger, decays to 0 with a fast time constant)
    pitch_env: Sample,
    // Previous gate value for rising-edge detection
    prev_gate: Sample,
    // Low-pass filter state used to derive the noise high-pass (input - lp = hp)
    noise_lp: Sample,
    // Random number generator for white noise
    rng: StdRng,
    // Optional seed stored so describe_config can report it
    seed: Option<u64>,
    // Default parameter values
    default_tune: Sample,
    default_tone: Sample,
    default_snappy: Sample,
    default_tone_decay: Sample,
    default_snappy_decay: Sample,
    default_noise_filter: Sample,
    default_pitch_sweep: Sample,
}

impl UGSnareDrum {
    /// Create a new UGSnareDrum with a random internal seed.
    pub fn new() -> Self {
        let actual_seed: u64 = rand::rng().random();
        Self::with_seed(actual_seed, None)
    }

    /// Create a new UGSnareDrum with an optional reproducible seed.
    /// When `seed` is `Some(n)`, the noise sequence is deterministic.
    pub fn new_seeded(seed: Option<u64>) -> Self {
        let actual_seed = seed.unwrap_or_else(|| rand::rng().random());
        Self::with_seed(actual_seed, seed)
    }

    fn with_seed(actual_seed: u64, seed: Option<u64>) -> Self {
        Self {
            tone_phase: 0.0,
            tone_env: 0.0,
            snappy_env: 0.0,
            pitch_env: 0.0,
            prev_gate: 0.0,
            noise_lp: 0.0,
            rng: StdRng::seed_from_u64(actual_seed),
            seed,
            default_tune: 180.0,
            default_tone: 0.7,
            default_snappy: 0.9,
            default_tone_decay: 3000.0,
            default_snappy_decay: 5000.0,
            default_noise_filter: 4000.0,
            default_pitch_sweep: 1.5,
        }
    }
}

impl Default for UGSnareDrum {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGSnareDrum {
    fn type_name(&self) -> &'static str {
        "UGSnareDrum"
    }

    fn input_names(&self) -> &[&'static str] {
        &[
            "gate",
            "tune",
            "tone",
            "snappy",
            "tone_decay",
            "snappy_decay",
            "noise_filter",
            "pitch_sweep",
        ]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "gate" => Some(0.0),
            "tune" => Some(self.default_tune),
            "tone" => Some(self.default_tone),
            "snappy" => Some(self.default_snappy),
            "tone_decay" => Some(self.default_tone_decay),
            "snappy_decay" => Some(self.default_snappy_decay),
            "noise_filter" => Some(self.default_noise_filter),
            "pitch_sweep" => Some(self.default_pitch_sweep),
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
        sample_rate: f32,
        _time_sample: usize,
    ) {
        let gate = inputs.first().copied().unwrap_or(&[]);
        let tune = inputs.get(1).copied().unwrap_or(&[]);
        let tone = inputs.get(2).copied().unwrap_or(&[]);
        let snappy = inputs.get(3).copied().unwrap_or(&[]);
        let tone_decay = inputs.get(4).copied().unwrap_or(&[]);
        let snappy_decay = inputs.get(5).copied().unwrap_or(&[]);
        let noise_filter = inputs.get(6).copied().unwrap_or(&[]);
        let pitch_sweep = inputs.get(7).copied().unwrap_or(&[]);

        let out = &mut outputs[0];
        let n = out.len();

        let dt = 1.0 / sample_rate;

        // Pitch sweep uses a fixed fast time constant (~20 ms at 44100 Hz, scaled by sample_rate).
        // exp(-LN_1000 / tau) where tau = 0.02 * sample_rate
        let pitch_decay_coeff = (-LN_1000 / (0.02 * sample_rate).max(1.0)).exp();

        for i in 0..n {
            // ── Read signal inputs, falling back to per-parameter defaults ────────
            let gate_v = gate.get(i).copied().unwrap_or(0.0);
            let tune_v = tune.get(i).copied().unwrap_or(self.default_tune);
            let tone_v = tone.get(i).copied().unwrap_or(self.default_tone);
            let snappy_v = snappy.get(i).copied().unwrap_or(self.default_snappy);
            let tone_decay_v = tone_decay
                .get(i)
                .copied()
                .unwrap_or(self.default_tone_decay)
                .max(1.0);
            let snappy_decay_v = snappy_decay
                .get(i)
                .copied()
                .unwrap_or(self.default_snappy_decay)
                .max(1.0);
            let noise_filter_v = noise_filter
                .get(i)
                .copied()
                .unwrap_or(self.default_noise_filter)
                .clamp(20.0, sample_rate * 0.45);
            let pitch_sweep_v = pitch_sweep
                .get(i)
                .copied()
                .unwrap_or(self.default_pitch_sweep)
                .max(1.0);

            // ── Rising-edge detection on gate ─────────────────────────────────────
            if gate_v > 0.5 && self.prev_gate <= 0.5 {
                self.tone_env = 1.0;
                self.snappy_env = 1.0;
                self.pitch_env = 1.0;
            }
            self.prev_gate = gate_v;

            // ── Tonal body: sine oscillator with exponential pitch sweep ──────────
            // Instantaneous frequency: starts at tune * pitch_sweep and sweeps
            // exponentially down toward tune as pitch_env decays.
            let freq = tune_v * (1.0 + (pitch_sweep_v - 1.0) * self.pitch_env);
            self.tone_phase += freq * dt;
            if self.tone_phase >= 1.0 {
                self.tone_phase -= 1.0;
            }
            let tone_out =
                (self.tone_phase * std::f32::consts::TAU).sin() * self.tone_env * tone_v;

            // ── Noise component: white noise through a 1-pole high-pass filter ────
            // HP coefficient: g = 2π·fc/sr (EMA lowpass → subtract for highpass)
            let raw_noise: Sample = self.rng.random_range(-1.0_f32..=1.0_f32);
            let g = (std::f32::consts::TAU * noise_filter_v / sample_rate).min(1.0);
            self.noise_lp += g * (raw_noise - self.noise_lp);
            let filtered_noise = raw_noise - self.noise_lp;
            let snappy_out = filtered_noise * self.snappy_env * snappy_v;

            // ── Advance exponential envelopes ─────────────────────────────────────
            // coeff = exp(-LN_1000 / decay_samples) gives -60 dB drop at the decay time.
            let tone_coeff = (-LN_1000 / tone_decay_v).exp();
            let snappy_coeff = (-LN_1000 / snappy_decay_v).exp();
            self.tone_env *= tone_coeff;
            self.snappy_env *= snappy_coeff;
            self.pitch_env *= pitch_decay_coeff;

            // ── Mix and apply analog-style soft saturation (tanh) ─────────────────
            // Scale by 0.5 to prevent clipping when both components are at full amplitude.
            let mixed = (tone_out + snappy_out) * 0.5;
            out[i] = mixed.tanh();
        }
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::UGClock;
    use crate::connect_many;
    use crate::register_many;
    use crate::util::UnitRate;

    /// Snare should have correct metadata.
    #[test]
    fn test_snare_drum_metadata() {
        let s = UGSnareDrum::new();
        assert_eq!(s.type_name(), "UGSnareDrum");
        assert_eq!(
            s.input_names(),
            &[
                "gate",
                "tune",
                "tone",
                "snappy",
                "tone_decay",
                "snappy_decay",
                "noise_filter",
                "pitch_sweep"
            ]
        );
        assert_eq!(s.output_names(), &["out"]);
    }

    /// Default input values should be sensible.
    #[test]
    fn test_snare_drum_default_inputs() {
        let s = UGSnareDrum::new();
        assert_eq!(s.default_input("gate"), Some(0.0));
        assert_eq!(s.default_input("tune"), Some(180.0));
        assert_eq!(s.default_input("tone"), Some(0.7));
        assert_eq!(s.default_input("snappy"), Some(0.9));
        assert_eq!(s.default_input("tone_decay"), Some(3000.0));
        assert_eq!(s.default_input("snappy_decay"), Some(5000.0));
        assert_eq!(s.default_input("noise_filter"), Some(4000.0));
        assert_eq!(s.default_input("pitch_sweep"), Some(1.5));
        assert_eq!(s.default_input("unknown"), None);
    }

    /// When no gate is present the output should stay silent.
    #[test]
    fn test_snare_drum_silent_without_trigger() {
        let mut g = GenGraph::new(44100.0, 64);
        register_many![g,
            "snare" => UGSnareDrum::new_seeded(Some(1)),
        ];
        g.process();
        let out = g.get_output_by_label("snare.out");
        for &s in out {
            assert_eq!(s, 0.0, "snare should be silent without a trigger");
        }
    }

    /// A gate trigger should produce non-zero output, and the output should
    /// decay over time (exponential amplitude envelope).
    #[test]
    fn test_snare_drum_trigger_and_decay() {
        // buffer_size must be a multiple of 8 (SIMD lane width).
        // Use short decay times (16 samples each) so the decay is clearly visible
        // within the 48-sample window.
        let mut g = GenGraph::new(100.0, 48);
        register_many![g,
            "clock" => UGClock::new(48.0, UnitRate::Samples),
            "snare" => UGSnareDrum::new_seeded(Some(42)),
            "tdec" => 16,
            "sdec" => 16,
        ];
        connect_many![g,
            "clock.out" -> "snare.gate",
            "tdec.out" -> "snare.tone_decay",
            "sdec.out" -> "snare.snappy_decay",
        ];

        g.process();

        let out = g.get_output_by_label("snare.out");

        // Compute average absolute amplitude over two windows.
        let early: f32 = out[0..8].iter().map(|x| x.abs()).sum::<f32>() / 8.0;
        let late: f32 = out[32..40].iter().map(|x| x.abs()).sum::<f32>() / 8.0;

        // There should be some output on the initial hit.
        assert!(early > 0.0, "snare should produce output on trigger");

        // The output should be quieter later as the envelopes decay.
        assert!(
            late < early,
            "snare output amplitude should decay: early={early}, late={late}"
        );
    }

    /// Seeded constructor should produce identical results when called twice.
    #[test]
    fn test_snare_drum_seeded_reproducible() {
        let mut g1 = GenGraph::new(44100.0, 32);
        register_many![g1,
            "clock" => UGClock::new(32.0, UnitRate::Samples),
            "snare" => UGSnareDrum::new_seeded(Some(99)),
        ];
        connect_many![g1,
            "clock.out" -> "snare.gate",
        ];
        g1.process();
        let out1 = g1.get_output_by_label("snare.out");

        let mut g2 = GenGraph::new(44100.0, 32);
        register_many![g2,
            "clock" => UGClock::new(32.0, UnitRate::Samples),
            "snare" => UGSnareDrum::new_seeded(Some(99)),
        ];
        connect_many![g2,
            "clock.out" -> "snare.gate",
        ];
        g2.process();
        let out2 = g2.get_output_by_label("snare.out");

        assert_eq!(out1, out2, "seeded snare should be reproducible");
    }

    /// Different seeds should produce different noise outputs.
    #[test]
    fn test_snare_drum_different_seeds_differ() {
        let mut g1 = GenGraph::new(44100.0, 32);
        register_many![g1,
            "clock" => UGClock::new(32.0, UnitRate::Samples),
            "snare" => UGSnareDrum::new_seeded(Some(1)),
        ];
        connect_many![g1,
            "clock.out" -> "snare.gate",
        ];
        g1.process();
        let out1 = g1.get_output_by_label("snare.out");

        let mut g2 = GenGraph::new(44100.0, 32);
        register_many![g2,
            "clock" => UGClock::new(32.0, UnitRate::Samples),
            "snare" => UGSnareDrum::new_seeded(Some(2)),
        ];
        connect_many![g2,
            "clock.out" -> "snare.gate",
        ];
        g2.process();
        let out2 = g2.get_output_by_label("snare.out");

        assert_ne!(
            out1, out2,
            "different seeds should produce different output"
        );
    }

    /// describe_config should return the seed string when a seed is provided.
    #[test]
    fn test_snare_drum_describe_config() {
        let seeded = UGSnareDrum::new_seeded(Some(7));
        assert_eq!(seeded.describe_config(), Some("seed = 7".to_string()));

        let unseeded = UGSnareDrum::new();
        assert_eq!(unseeded.describe_config(), None);
    }
}
