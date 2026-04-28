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
// UGBassDrum

/// An analog-style bass drum inspired by classic 808/909 circuits.
///
/// The drum uses a decaying sine body with exponential pitch sweep and a short
/// click transient. All controls are signal inputs for per-hit modulation.
///
/// Inputs:
///   0 gate         - Trigger input; rising edge (≤0.5 → >0.5) fires the drum.
///   1 tune         - Base oscillator frequency in Hz. Default: 55.0
///   2 decay        - Body decay in samples. Default: 9000.0
///   3 punch        - Start pitch multiplier for sweep [>=1]. Default: 2.8
///   4 sweep_decay  - Pitch sweep decay in samples. Default: 1200.0
///   5 click        - Click transient amount [0..1]. Default: 0.2
///   6 tone         - Body amount [0..1]. Default: 1.0
///   7 drive        - Output drive before tanh saturation. Default: 1.3
///
/// Outputs:
///   0 out - Bass drum output in approximately [-1..1].
pub struct UGBassDrum {
    phase: Sample,
    click_phase: Sample,
    amp_env: Sample,
    pitch_env: Sample,
    click_env: Sample,
    prev_gate: Sample,
    default_tune: Sample,
    default_decay: Sample,
    default_punch: Sample,
    default_sweep_decay: Sample,
    default_click: Sample,
    default_tone: Sample,
    default_drive: Sample,
}

impl UGBassDrum {
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            click_phase: 0.0,
            amp_env: 0.0,
            pitch_env: 0.0,
            click_env: 0.0,
            prev_gate: 0.0,
            default_tune: 55.0,
            default_decay: 9000.0,
            default_punch: 2.8,
            default_sweep_decay: 1200.0,
            default_click: 0.2,
            default_tone: 1.0,
            default_drive: 1.3,
        }
    }
}

impl Default for UGBassDrum {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGBassDrum {
    fn type_name(&self) -> &'static str {
        "UGBassDrum"
    }

    fn input_names(&self) -> &[&'static str] {
        &[
            "gate",
            "tune",
            "decay",
            "punch",
            "sweep_decay",
            "click",
            "tone",
            "drive",
        ]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "gate" => Some(0.0),
            "tune" => Some(self.default_tune),
            "decay" => Some(self.default_decay),
            "punch" => Some(self.default_punch),
            "sweep_decay" => Some(self.default_sweep_decay),
            "click" => Some(self.default_click),
            "tone" => Some(self.default_tone),
            "drive" => Some(self.default_drive),
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
        let gate = inputs.first().copied().unwrap_or(&[]);
        let tune = inputs.get(1).copied().unwrap_or(&[]);
        let decay = inputs.get(2).copied().unwrap_or(&[]);
        let punch = inputs.get(3).copied().unwrap_or(&[]);
        let sweep_decay = inputs.get(4).copied().unwrap_or(&[]);
        let click = inputs.get(5).copied().unwrap_or(&[]);
        let tone = inputs.get(6).copied().unwrap_or(&[]);
        let drive = inputs.get(7).copied().unwrap_or(&[]);

        let out = &mut outputs[0];
        let dt = 1.0 / sample_rate;
        // 1.5 ms click decay: exp(-LN_1000 / tau_samples) reaches ~-60 dB at tau.
        // Using LN_1000 keeps envelope shapes consistent with other drum decays.
        let click_decay_coeff = (-LN_1000 / (0.0015 * sample_rate).max(1.0)).exp();

        for (i, o) in out.iter_mut().enumerate() {
            let gate_v = gate.get(i).copied().unwrap_or(0.0);
            let tune_v = tune
                .get(i)
                .copied()
                .unwrap_or(self.default_tune)
                // Keep oscillator well below Nyquist (0.5*sr): 0.45 leaves headroom to
                // reduce aliasing under modulation/saturation.
                .clamp(20.0, sample_rate * 0.45);
            let decay_v = decay.get(i).copied().unwrap_or(self.default_decay).max(1.0);
            let punch_v = punch.get(i).copied().unwrap_or(self.default_punch).max(1.0);
            let sweep_decay_v = sweep_decay
                .get(i)
                .copied()
                .unwrap_or(self.default_sweep_decay)
                .max(1.0);
            let click_v = click
                .get(i)
                .copied()
                .unwrap_or(self.default_click)
                .clamp(0.0, 1.0);
            let tone_v = tone
                .get(i)
                .copied()
                .unwrap_or(self.default_tone)
                .clamp(0.0, 1.0);
            let drive_v = drive.get(i).copied().unwrap_or(self.default_drive).max(0.0);

            if gate_v > 0.5 && self.prev_gate <= 0.5 {
                self.amp_env = 1.0;
                self.pitch_env = 1.0;
                self.click_env = 1.0;
                self.click_phase = 0.0;
            }
            self.prev_gate = gate_v;

            let freq = tune_v * (1.0 + (punch_v - 1.0) * self.pitch_env);
            self.phase += freq * dt;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            let sine = (self.phase * std::f32::consts::TAU).sin();
            // 0.85/0.15 blend adds a small sine^3 term (odd-harmonic warmth) while
            // keeping the fundamental dominant; no DC shift is introduced.
            let body = (0.85 * sine + 0.15 * sine.powi(3)) * self.amp_env * tone_v;

            // Click oscillator is intentionally very high: ~45x tune gives a short
            // beater-like attack burst in the low-kHz range for typical kick tuning.
            let click_freq = (tune_v * 45.0).min(sample_rate * 0.45);
            self.click_phase += click_freq * dt;
            if self.click_phase >= 1.0 {
                self.click_phase -= 1.0;
            }
            let click_out = (self.click_phase * std::f32::consts::TAU).sin()
                * self.click_env
                * click_v;

            let amp_coeff = (-LN_1000 / decay_v).exp();
            let sweep_coeff = (-LN_1000 / sweep_decay_v).exp();
            self.amp_env *= amp_coeff;
            self.pitch_env *= sweep_coeff;
            self.click_env *= click_decay_coeff;

            *o = ((body + click_out * 0.6) * drive_v).tanh();
        }
    }
}

//------------------------------------------------------------------------------
// UGHighHat

/// Inharmonic frequency ratios of the six oscillators in the TR-808 hi-hat circuit.
/// Derived from the 808 service manual resistor network (approximate Hz values:
/// 3969, 4420, 5280, 5562, 6225, 8662), normalized to the lowest frequency.
const HAT_OSC_RATIOS: [f32; 6] = [1.0, 1.114, 1.330, 1.401, 1.568, 2.182];

/// An analog-style hi-hat modeled after the Roland TR-808/909 circuits.
///
/// The metallic timbre is generated by mixing six square-wave oscillators tuned
/// to inharmonic ratios (matching the 808 resistor network), fed through a
/// state-variable band-pass filter and shaped by an exponential amplitude
/// envelope. White noise can be blended in for extra sizzle. Soft tanh
/// saturation provides analog warmth.
///
/// All parameters are signal inputs, enabling dynamic per-hit modulation.
///
/// Inputs:
///   0 gate    - Trigger input; rising edge (≤0.5 → >0.5) fires the hat.
///   1 tune    - Base frequency for the oscillator cluster in Hz. Default: 3969.0
///   2 decay   - Amplitude decay in samples. Default: 4000.0
///               (≈ 90 ms at 44 100 Hz; use ~800 for a closed-hat sound)
///   3 tone    - Band-pass filter centre frequency in Hz. Default: 8000.0
///   4 accent  - Initial amplitude / velocity [0..1]. Default: 0.8
///   5 noise   - White-noise blend [0..1]; 0 = pure metallic, 1 = pure noise. Default: 0.2
///   6 drive   - Output drive before tanh saturation. Default: 1.2
///
/// Outputs:
///   0 out - Hi-hat output in approximately [-1..1].
pub struct UGHighHat {
    // 6 inharmonic square-wave oscillators (phases in 0..1)
    osc_phases: [Sample; 6],
    // Exponential amplitude envelope (set to accent level on trigger, decays to 0)
    amp_env: Sample,
    // State-variable band-pass filter state
    bp_low: Sample,
    bp_band: Sample,
    // Previous gate value for rising-edge detection
    prev_gate: Sample,
    // Random number generator for the white-noise blend
    rng: StdRng,
    // Optional seed stored so describe_config can report it
    seed: Option<u64>,
    // Default parameter values
    default_tune: Sample,
    default_decay: Sample,
    default_tone: Sample,
    default_accent: Sample,
    default_noise: Sample,
    default_drive: Sample,
}

impl UGHighHat {
    /// Create a new UGHighHat. If `seed` is `None`, a random seed is used for the noise
    /// sequence; `Some(n)` makes the sequence deterministic and reproducible.
    pub fn new(seed: Option<u64>) -> Self {
        let actual_seed = seed.unwrap_or_else(|| rand::rng().random());
        Self {
            osc_phases: [0.0; 6],
            amp_env: 0.0,
            bp_low: 0.0,
            bp_band: 0.0,
            prev_gate: 0.0,
            rng: StdRng::seed_from_u64(actual_seed),
            seed,
            default_tune: 3969.0,
            default_decay: 4000.0,
            default_tone: 8000.0,
            default_accent: 0.8,
            default_noise: 0.2,
            default_drive: 1.2,
        }
    }
}

impl Default for UGHighHat {
    fn default() -> Self {
        Self::new(None)
    }
}

impl UGen for UGHighHat {
    fn type_name(&self) -> &'static str {
        "UGHighHat"
    }

    fn input_names(&self) -> &[&'static str] {
        &["gate", "tune", "decay", "tone", "accent", "noise", "drive"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "gate" => Some(0.0),
            "tune" => Some(self.default_tune),
            "decay" => Some(self.default_decay),
            "tone" => Some(self.default_tone),
            "accent" => Some(self.default_accent),
            "noise" => Some(self.default_noise),
            "drive" => Some(self.default_drive),
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
        let decay = inputs.get(2).copied().unwrap_or(&[]);
        let tone = inputs.get(3).copied().unwrap_or(&[]);
        let accent = inputs.get(4).copied().unwrap_or(&[]);
        let noise = inputs.get(5).copied().unwrap_or(&[]);
        let drive = inputs.get(6).copied().unwrap_or(&[]);

        let out = &mut outputs[0];
        let dt = 1.0 / sample_rate;

        // Fixed internal Q for the state-variable BP filter.
        // Q ≈ 3 gives a moderately resonant, characteristically metallic band-pass.
        // Damping coefficient R = 1 / (2 * Q).
        let bp_q = 3.0_f32;
        let bp_r = 1.0 / (2.0 * bp_q);

        for (i, o) in out.iter_mut().enumerate() {
            let gate_v = gate.get(i).copied().unwrap_or(0.0);
            let tune_v = tune
                .get(i)
                .copied()
                .unwrap_or(self.default_tune)
                .clamp(20.0, (sample_rate * 0.45 / HAT_OSC_RATIOS[5]).max(20.0));
            let decay_v = decay.get(i).copied().unwrap_or(self.default_decay).max(1.0);
            let tone_v = tone
                .get(i)
                .copied()
                .unwrap_or(self.default_tone)
                .clamp(20.0, sample_rate * 0.45);
            let accent_v = accent
                .get(i)
                .copied()
                .unwrap_or(self.default_accent)
                .clamp(0.0, 1.0);
            let noise_v = noise
                .get(i)
                .copied()
                .unwrap_or(self.default_noise)
                .clamp(0.0, 1.0);
            let drive_v = drive.get(i).copied().unwrap_or(self.default_drive).max(0.0);

            // ── Rising-edge detection on gate ─────────────────────────────────────
            if gate_v > 0.5 && self.prev_gate <= 0.5 {
                self.amp_env = accent_v;
            }
            self.prev_gate = gate_v;

            // ── Metallic source: 6 square oscillators at inharmonic ratios ────────
            let mut metallic = 0.0_f32;
            for (k, ratio) in HAT_OSC_RATIOS.iter().enumerate() {
                let freq = (tune_v * ratio).min(sample_rate * 0.45);
                self.osc_phases[k] += freq * dt;
                if self.osc_phases[k] >= 1.0 {
                    self.osc_phases[k] -= 1.0;
                }
                // Square wave: +1 for first half of cycle, -1 for second half
                metallic += if self.osc_phases[k] < 0.5 { 1.0 } else { -1.0 };
            }
            metallic /= 6.0; // normalize to [-1..1]

            // ── White noise blend ─────────────────────────────────────────────────
            let white: Sample = self.rng.random_range(-1.0_f32..=1.0_f32);
            let raw = metallic * (1.0 - noise_v) + white * noise_v;

            // ── State-variable band-pass filter (Euler form) ──────────────────────
            // f0 = 2π·fc/sr (clamped to avoid instability near Nyquist)
            let f0 = (std::f32::consts::TAU * tone_v / sample_rate).min(1.0);
            let hp = raw - self.bp_low - 2.0 * bp_r * self.bp_band;
            self.bp_band += f0 * hp;
            self.bp_low += f0 * self.bp_band;
            let bp_out = self.bp_band;

            // ── Apply amplitude envelope ──────────────────────────────────────────
            let sig = bp_out * self.amp_env;

            // ── Advance exponential envelope ──────────────────────────────────────
            // coeff = exp(-LN_1000 / decay_samples) gives -60 dB drop at decay time.
            let coeff = (-LN_1000 / decay_v).exp();
            self.amp_env *= coeff;

            // ── Soft tanh saturation for analog warmth ────────────────────────────
            *o = (sig * drive_v).tanh();
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
    use crate::graph_from_chain_expression;
    use crate::register_many;
    use crate::util::UnitRate;

    /// Bass drum should have correct metadata.
    #[test]
    fn test_bass_drum_metadata() {
        let b = UGBassDrum::new();
        assert_eq!(b.type_name(), "UGBassDrum");
        assert_eq!(
            b.input_names(),
            &[
                "gate",
                "tune",
                "decay",
                "punch",
                "sweep_decay",
                "click",
                "tone",
                "drive"
            ]
        );
        assert_eq!(b.output_names(), &["out"]);
    }

    /// Default input values should be sensible.
    #[test]
    fn test_bass_drum_default_inputs() {
        let b = UGBassDrum::new();
        assert_eq!(b.default_input("gate"), Some(0.0));
        assert_eq!(b.default_input("tune"), Some(55.0));
        assert_eq!(b.default_input("decay"), Some(9000.0));
        assert_eq!(b.default_input("punch"), Some(2.8));
        assert_eq!(b.default_input("sweep_decay"), Some(1200.0));
        assert_eq!(b.default_input("click"), Some(0.2));
        assert_eq!(b.default_input("tone"), Some(1.0));
        assert_eq!(b.default_input("drive"), Some(1.3));
        assert_eq!(b.default_input("unknown"), None);
    }

    /// Without a trigger, bass drum should remain silent.
    #[test]
    fn test_bass_drum_silent_without_trigger() {
        let mut g = GenGraph::new(44100.0, 64);
        register_many![g,
            "kick" => UGBassDrum::new(),
        ];
        g.process();
        let out = g.get_output_by_label("kick.out");
        for &s in out {
            assert_eq!(s, 0.0, "bass drum should be silent without a trigger");
        }
    }

    /// Triggered bass drum should produce output and decay over time.
    #[test]
    fn test_bass_drum_trigger_and_decay() {
        let mut g = GenGraph::new(100.0, 48);
        register_many![g,
            "clock" => UGClock::new(48.0, UnitRate::Samples),
            "kick" => UGBassDrum::new(),
            "dec" => 16,
            "sdec" => 8,
        ];
        connect_many![g,
            "clock.out" -> "kick.gate",
            "dec.out" -> "kick.decay",
            "sdec.out" -> "kick.sweep_decay",
        ];

        g.process();

        let out = g.get_output_by_label("kick.out");
        let early: f32 = out[0..8].iter().map(|x| x.abs()).sum::<f32>() / 8.0;
        let late: f32 = out[32..40].iter().map(|x| x.abs()).sum::<f32>() / 8.0;

        assert!(early > 0.0, "bass drum should produce output on trigger");
        assert!(
            late < early,
            "bass drum output should decay: early={early}, late={late}"
        );
    }

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

    // ── UGHighHat tests ──────────────────────────────────────────────────────

    /// Hi-hat should have correct metadata.
    #[test]
    fn test_high_hat_metadata() {
        let h = UGHighHat::new(None);
        assert_eq!(h.type_name(), "UGHighHat");
        assert_eq!(
            h.input_names(),
            &["gate", "tune", "decay", "tone", "accent", "noise", "drive"]
        );
        assert_eq!(h.output_names(), &["out"]);
    }

    /// Default input values should be sensible.
    #[test]
    fn test_high_hat_default_inputs() {
        let h = UGHighHat::new(None);
        assert_eq!(h.default_input("gate"), Some(0.0));
        assert_eq!(h.default_input("tune"), Some(3969.0));
        assert_eq!(h.default_input("decay"), Some(4000.0));
        assert_eq!(h.default_input("tone"), Some(8000.0));
        assert_eq!(h.default_input("accent"), Some(0.8));
        assert_eq!(h.default_input("noise"), Some(0.2));
        assert_eq!(h.default_input("drive"), Some(1.2));
        assert_eq!(h.default_input("unknown"), None);
    }

    /// Without a trigger the hi-hat should remain silent.
    #[test]
    fn test_high_hat_silent_without_trigger() {
        let mut g = graph_from_chain_expression(
            "HighHat(seed=1) => hat",
            44100.0,
            64,
        )
        .unwrap();
        g.process();
        let out = g.get_output_by_label("hat.out");
        for &s in out {
            assert_eq!(s, 0.0, "hi-hat should be silent without a trigger");
        }
    }

    /// A gate trigger should produce non-zero output that decays over time.
    #[test]
    fn test_high_hat_trigger_and_decay() {
        let mut g = graph_from_chain_expression(
            "Clock(value=48.0, mode=Samples) => clock \
             | HighHat(seed=42) => hat \
             | 16 => dec \
             | clock ->:gate hat \
             | dec ->:decay hat",
            100.0,
            48,
        )
        .unwrap();

        g.process();

        let out = g.get_output_by_label("hat.out");
        let early: f32 = out[0..8].iter().map(|x| x.abs()).sum::<f32>() / 8.0;
        let late: f32 = out[32..40].iter().map(|x| x.abs()).sum::<f32>() / 8.0;

        assert!(early > 0.0, "hi-hat should produce output on trigger");
        assert!(
            late < early,
            "hi-hat output should decay: early={early}, late={late}"
        );
    }

    /// Seeded constructor should produce identical results when called twice.
    #[test]
    fn test_high_hat_seeded_reproducible() {
        let chain = "Clock(value=32.0, mode=Samples) => clock \
                     | HighHat(seed=99) => hat \
                     | clock ->:gate hat";

        let mut g1 = graph_from_chain_expression(chain, 44100.0, 32).unwrap();
        g1.process();
        let out1 = g1.get_output_by_label("hat.out").to_vec();

        let mut g2 = graph_from_chain_expression(chain, 44100.0, 32).unwrap();
        g2.process();
        let out2 = g2.get_output_by_label("hat.out").to_vec();

        assert_eq!(out1, out2, "seeded hi-hat should be reproducible");
    }

    /// Different seeds should produce different noise-blended outputs.
    #[test]
    fn test_high_hat_different_seeds_differ() {
        let chain1 = "Clock(value=32.0, mode=Samples) => clock \
                      | HighHat(seed=1) => hat \
                      | clock ->:gate hat";
        let chain2 = "Clock(value=32.0, mode=Samples) => clock \
                      | HighHat(seed=2) => hat \
                      | clock ->:gate hat";

        let mut g1 = graph_from_chain_expression(chain1, 44100.0, 32).unwrap();
        g1.process();
        let out1 = g1.get_output_by_label("hat.out").to_vec();

        let mut g2 = graph_from_chain_expression(chain2, 44100.0, 32).unwrap();
        g2.process();
        let out2 = g2.get_output_by_label("hat.out").to_vec();

        assert_ne!(
            out1, out2,
            "different seeds should produce different hi-hat output"
        );
    }

    /// describe_config should return the seed string when a seed is provided.
    #[test]
    fn test_high_hat_describe_config() {
        let seeded = UGHighHat::new(Some(7));
        assert_eq!(seeded.describe_config(), Some("seed = 7".to_string()));

        let unseeded = UGHighHat::new(None);
        assert_eq!(unseeded.describe_config(), None);
    }
}
