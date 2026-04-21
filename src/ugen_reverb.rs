use crate::Sample;
use crate::UGen;

const DEFAULT_DECAY: f32 = 0.6;
const DEFAULT_PRE_DELAY_MS: f32 = 20.0;
const DEFAULT_MIX: f32 = 0.35;
const DEFAULT_SIZE: f32 = 1.0;
const DEFAULT_DIFFUSION: f32 = 0.75;
const DEFAULT_DAMPING_HZ: f32 = 7000.0;

const MAX_PRE_DELAY_MS: f32 = 500.0;
// Mix a small amount of each channel into the opposite tank to decorrelate tails.
const CROSSFEED_GAIN: f32 = 0.2;

#[derive(Debug)]
struct DelayLine {
    buffer: Vec<f32>,
    write_idx: usize,
}

impl DelayLine {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size.max(1)],
            write_idx: 0,
        }
    }

    #[inline]
    fn read(&self, delay_samples: usize) -> f32 {
        let len = self.buffer.len();
        let delay = delay_samples.min(len - 1);
        let read_idx = (self.write_idx + len - delay) % len;
        self.buffer[read_idx]
    }

    #[inline]
    fn write_advance(&mut self, value: f32) {
        self.buffer[self.write_idx] = value;
        self.write_idx += 1;
        if self.write_idx == self.buffer.len() {
            self.write_idx = 0;
        }
    }
}

#[derive(Debug)]
struct Comb {
    delay: DelayLine,
    damp_state: f32,
}

impl Comb {
    fn new(max_delay_samples: usize) -> Self {
        Self {
            delay: DelayLine::new(max_delay_samples + 1),
            damp_state: 0.0,
        }
    }

    #[inline]
    fn process(
        &mut self,
        input: f32,
        delay_samples: usize,
        feedback: f32,
        damping_coeff: f32,
    ) -> f32 {
        let delayed = self.delay.read(delay_samples.max(1));
        self.damp_state += damping_coeff * (delayed - self.damp_state);
        self.delay
            .write_advance(input + self.damp_state * feedback.clamp(0.0, 0.99));
        delayed
    }
}

#[derive(Debug)]
struct AllPass {
    delay: DelayLine,
}

impl AllPass {
    fn new(max_delay_samples: usize) -> Self {
        Self {
            delay: DelayLine::new(max_delay_samples + 1),
        }
    }

    #[inline]
    fn process(&mut self, input: f32, delay_samples: usize, feedback: f32) -> f32 {
        let g = feedback.clamp(0.0, 0.95);
        let delayed = self.delay.read(delay_samples.max(1));
        let y = delayed - g * input;
        self.delay.write_advance(input + g * delayed);
        y
    }
}

fn damping_coeff(high_cut_hz: f32, sample_rate: f32) -> f32 {
    let max_cut = (sample_rate * 0.5 - 1.0).max(1.0);
    let min_cut = 1.0_f32.min(max_cut);
    let fc = high_cut_hz.clamp(min_cut, max_cut);
    1.0 - (-2.0 * std::f32::consts::PI * fc / sample_rate).exp()
}

/// Stereo reverb UGen with controls for decay time, pre-delay, dry/wet mix,
/// room size, diffusion, and damping (high-cut).
pub struct UGReverb {
    pre_l: DelayLine,
    pre_r: DelayLine,
    comb_l: Vec<Comb>,
    comb_r: Vec<Comb>,
    allpass_l: Vec<AllPass>,
    allpass_r: Vec<AllPass>,
}

impl UGReverb {
    pub fn new() -> Self {
        // Buffers are sized for 48 kHz and up to `MAX_PRE_DELAY_MS`.
        // At sample rates above 48 kHz, delay taps clamp to buffer capacity, so effective
        // delay times become somewhat shorter than requested and the room character is tighter.
        let max_sr = 48_000.0_f32;
        let max_pre_samples = ((max_sr * MAX_PRE_DELAY_MS) / 1000.0).ceil() as usize + 2;
        let max_size = 1.5_f32;

        let base_comb_l = [1116, 1188, 1277, 1356];
        let base_comb_r = [1139, 1211, 1300, 1379];
        let base_ap_l = [556, 441];
        let base_ap_r = [579, 464];

        let max_comb = base_comb_r
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .max(base_comb_l.iter().copied().max().unwrap_or(0));
        let max_ap = base_ap_r
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .max(base_ap_l.iter().copied().max().unwrap_or(0));

        let comb_capacity =
            ((max_comb as f32 * max_size * max_sr / 44_100.0).ceil() as usize) + 4;
        let ap_capacity =
            ((max_ap as f32 * max_size * max_sr / 44_100.0).ceil() as usize) + 4;

        Self {
            pre_l: DelayLine::new(max_pre_samples),
            pre_r: DelayLine::new(max_pre_samples),
            comb_l: base_comb_l
                .iter()
                .map(|_| Comb::new(comb_capacity))
                .collect(),
            comb_r: base_comb_r
                .iter()
                .map(|_| Comb::new(comb_capacity))
                .collect(),
            allpass_l: base_ap_l
                .iter()
                .map(|_| AllPass::new(ap_capacity))
                .collect(),
            allpass_r: base_ap_r
                .iter()
                .map(|_| AllPass::new(ap_capacity))
                .collect(),
        }
    }
}

impl Default for UGReverb {
    fn default() -> Self {
        Self::new()
    }
}

impl UGen for UGReverb {
    fn type_name(&self) -> &'static str {
        "UGReverb"
    }

    fn input_names(&self) -> &[&'static str] {
        &[
            "in_l",
            "in_r",
            "decay",
            "pre_delay",
            "mix",
            "size",
            "diffusion",
            "damping",
        ]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out_l", "out_r"]
    }

    fn default_input(&self, name: &str) -> Option<Sample> {
        match name {
            "in_l" | "in_r" => Some(0.0),
            "decay" => Some(DEFAULT_DECAY),
            "pre_delay" => Some(DEFAULT_PRE_DELAY_MS),
            "mix" => Some(DEFAULT_MIX),
            "size" => Some(DEFAULT_SIZE),
            "diffusion" => Some(DEFAULT_DIFFUSION),
            "damping" => Some(DEFAULT_DAMPING_HZ),
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
        let in_l = inputs.first().copied().unwrap_or(&[]);
        let in_r = inputs.get(1).copied().unwrap_or(&[]);
        let decay = inputs.get(2).copied().unwrap_or(&[]);
        let pre_delay = inputs.get(3).copied().unwrap_or(&[]);
        let mix = inputs.get(4).copied().unwrap_or(&[]);
        let size = inputs.get(5).copied().unwrap_or(&[]);
        let diffusion = inputs.get(6).copied().unwrap_or(&[]);
        let damping = inputs.get(7).copied().unwrap_or(&[]);

        let (left, right) = outputs.split_at_mut(1);
        let out_l = &mut left[0];
        let out_r = &mut right[0];

        let comb_base_l = [1116_usize, 1188, 1277, 1356];
        let comb_base_r = [1139_usize, 1211, 1300, 1379];
        let ap_base_l = [556_usize, 441];
        let ap_base_r = [579_usize, 464];
        let sr_ratio = sample_rate / 44_100.0;

        for i in 0..out_l.len() {
            let dry_l = in_l.get(i).copied().unwrap_or(0.0);
            let dry_r = in_r.get(i).copied().unwrap_or(0.0);

            let decay_v = decay
                .get(i)
                .copied()
                .unwrap_or(DEFAULT_DECAY)
                .clamp(0.0, 0.98);
            let pre_ms = pre_delay
                .get(i)
                .copied()
                .unwrap_or(DEFAULT_PRE_DELAY_MS)
                .clamp(0.0, MAX_PRE_DELAY_MS);
            let mix_v = mix.get(i).copied().unwrap_or(DEFAULT_MIX).clamp(0.0, 1.0);
            let size_v = size.get(i).copied().unwrap_or(DEFAULT_SIZE).clamp(0.5, 1.5);
            let diffusion_v = diffusion
                .get(i)
                .copied()
                .unwrap_or(DEFAULT_DIFFUSION)
                .clamp(0.0, 0.95);
            let damping_v = damping.get(i).copied().unwrap_or(DEFAULT_DAMPING_HZ);

            let pre_samp = ((pre_ms * sample_rate) / 1000.0).round() as usize;
            let (pdl, pdr) = if pre_samp == 0 {
                (dry_l, dry_r)
            } else {
                let l = self.pre_l.read(pre_samp);
                let r = self.pre_r.read(pre_samp);
                (l, r)
            };
            self.pre_l.write_advance(dry_l);
            self.pre_r.write_advance(dry_r);

            // Feed a little of each channel into the opposite tank so each side contains
            // different reflection histories; this avoids dual-mono tails and increases width.
            let tank_in_l = pdl + pdr * CROSSFEED_GAIN;
            let tank_in_r = pdr + pdl * CROSSFEED_GAIN;
            let damp_coeff = damping_coeff(damping_v, sample_rate);

            let mut wet_l = 0.0;
            for (idx, comb) in self.comb_l.iter_mut().enumerate() {
                let delay = ((comb_base_l[idx] as f32 * size_v * sr_ratio).round()
                    as usize)
                    .max(1);
                wet_l += comb.process(tank_in_l, delay, decay_v, damp_coeff);
            }
            wet_l /= self.comb_l.len() as f32;

            let mut wet_r = 0.0;
            for (idx, comb) in self.comb_r.iter_mut().enumerate() {
                let delay = ((comb_base_r[idx] as f32 * size_v * sr_ratio).round()
                    as usize)
                    .max(1);
                wet_r += comb.process(tank_in_r, delay, decay_v, damp_coeff);
            }
            wet_r /= self.comb_r.len() as f32;

            for (idx, ap) in self.allpass_l.iter_mut().enumerate() {
                let delay =
                    ((ap_base_l[idx] as f32 * size_v * sr_ratio).round() as usize).max(1);
                wet_l = ap.process(wet_l, delay, diffusion_v);
            }
            for (idx, ap) in self.allpass_r.iter_mut().enumerate() {
                let delay =
                    ((ap_base_r[idx] as f32 * size_v * sr_ratio).round() as usize).max(1);
                wet_r = ap.process(wet_r, delay, diffusion_v);
            }

            let dry_mix = 1.0 - mix_v;
            out_l[i] = dry_l * dry_mix + wet_l * mix_v;
            out_r[i] = dry_r * dry_mix + wet_r * mix_v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::Recorder;
    use crate::UGConst;

    fn max_abs(values: &[f32]) -> f32 {
        values.iter().fold(0.0_f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn test_reverb_metadata_and_defaults() {
        let r = UGReverb::new();
        assert_eq!(r.type_name(), "UGReverb");
        assert_eq!(
            r.input_names(),
            &[
                "in_l",
                "in_r",
                "decay",
                "pre_delay",
                "mix",
                "size",
                "diffusion",
                "damping"
            ]
        );
        assert_eq!(r.output_names(), &["out_l", "out_r"]);
        assert_eq!(r.default_input("decay"), Some(DEFAULT_DECAY));
        assert_eq!(r.default_input("pre_delay"), Some(DEFAULT_PRE_DELAY_MS));
        assert_eq!(r.default_input("mix"), Some(DEFAULT_MIX));
        assert_eq!(r.default_input("size"), Some(DEFAULT_SIZE));
        assert_eq!(r.default_input("diffusion"), Some(DEFAULT_DIFFUSION));
        assert_eq!(r.default_input("damping"), Some(DEFAULT_DAMPING_HZ));
        assert_eq!(r.default_input("unknown"), None);
    }

    #[test]
    fn test_reverb_dry_mix_passthrough() {
        let mut g = GenGraph::new(44_100.0, 64);
        g.add_node("l", Box::new(UGConst::new(0.25)));
        g.add_node("r", Box::new(UGConst::new(-0.5)));
        g.add_node("mix", Box::new(UGConst::new(0.0)));
        g.add_node("rev", Box::new(UGReverb::new()));
        g.connect("l.out", "rev.in_l");
        g.connect("r.out", "rev.in_r");
        g.connect("mix.out", "rev.mix");
        g.process();

        assert_eq!(g.get_output_by_label("rev.out_l"), vec![0.25; 64]);
        assert_eq!(g.get_output_by_label("rev.out_r"), vec![-0.5; 64]);
    }

    #[test]
    fn test_reverb_stereo_tail_generation() {
        let mut g = GenGraph::new(44_100.0, 256);
        g.add_node("imp_l", Box::new(UGConst::new(1.0)));
        g.add_node("imp_r", Box::new(UGConst::new(0.0)));
        g.add_node("mix", Box::new(UGConst::new(1.0)));
        g.add_node("pre", Box::new(UGConst::new(0.0)));
        g.add_node("size", Box::new(UGConst::new(0.5)));
        g.add_node("diff", Box::new(UGConst::new(0.8)));
        g.add_node("rev", Box::new(UGReverb::new()));
        g.connect("imp_l.out", "rev.in_l");
        g.connect("imp_r.out", "rev.in_r");
        g.connect("mix.out", "rev.mix");
        g.connect("pre.out", "rev.pre_delay");
        g.connect("size.out", "rev.size");
        g.connect("diff.out", "rev.diffusion");

        let r1 = Recorder::from_samples(g, None, 6000);
        let l = r1.get_output_by_label("rev.out_l");
        let r = r1.get_output_by_label("rev.out_r");
        let max_l = max_abs(l);
        let max_r = max_abs(r);
        assert!(max_l > 1e-4, "max_l={max_l}");
        assert!(max_r > 1e-6, "max_r={max_r}");
    }
}
