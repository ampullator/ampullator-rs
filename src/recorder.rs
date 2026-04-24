use crate::Sample;
use std::collections::HashMap;
// use crate::UGen;
use crate::GenGraph;
use std::collections::HashSet;
use std::path::Path;

use std::io::Write;
use std::process::Command;
use tempfile::NamedTempFile;

/// Selects the sample format and bit depth used when writing a WAV file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavFormat {
    /// 32-bit IEEE floating-point (lossless for `f32` samples).
    Float32,
    /// 16-bit signed integer (quantises samples in `[-1.0, 1.0]` to `[-32768, 32767]`).
    Int16,
    /// 24-bit signed integer (quantises samples in `[-1.0, 1.0]` to `[-8388608, 8388607]`).
    Int24,
}

impl TryFrom<u16> for WavFormat {
    type Error = String;

    fn try_from(bits: u16) -> Result<Self, Self::Error> {
        match bits {
            16 => Ok(WavFormat::Int16),
            24 => Ok(WavFormat::Int24),
            32 => Ok(WavFormat::Float32),
            _ => Err(format!("Unsupported bit depth: {bits}")),
        }
    }
}

pub struct Recorder {
    sample_rate: f32,
    recorded: HashMap<String, Vec<Sample>>,
    output_names: Vec<String>,
}

impl Recorder {
    pub fn from_samples(
        mut graph: GenGraph,
        output_labels: Option<Vec<String>>,
        total_samples: usize,
    ) -> Self {
        let sample_rate = graph.sample_rate;
        let buffer_size = graph.buffer_size;
        let mut recorded: HashMap<String, Vec<Sample>> = HashMap::new();
        let mut collected_labels: HashSet<String> = HashSet::new();

        match output_labels {
            Some(ref labels) => {
                for label in labels {
                    recorded.insert(label.clone(), Vec::with_capacity(total_samples));
                    collected_labels.insert(label.clone());
                }
            }
            None => {
                for (label, _) in graph.get_outputs() {
                    recorded.insert(label.clone(), Vec::with_capacity(total_samples));
                    collected_labels.insert(label);
                }
            }
        }

        let iterations = total_samples.div_ceil(buffer_size);

        for _ in 0..iterations {
            graph.process();
            for label in &collected_labels {
                let buffer = graph.get_output_by_label(label);
                recorded.get_mut(label).unwrap().extend_from_slice(buffer);
            }
        }

        // this seems to trim down to the total size
        for samples in recorded.values_mut() {
            samples.truncate(total_samples);
        }

        let output_names = match output_labels {
            Some(labels) => labels,
            None => graph.get_node_output_names(),
        };

        Self {
            sample_rate,
            recorded,
            output_names,
        }
    }

    pub fn from_duration(
        graph: GenGraph,
        output_labels: Option<Vec<String>>,
        duration_seconds: f32,
    ) -> Self {
        let total_samples = (duration_seconds * graph.sample_rate).round() as usize;
        Self::from_samples(graph, output_labels, total_samples)
    }

    //--------------------------------------------------------------------------
    pub fn get_shape(&self) -> (usize, usize) {
        let channels = self.recorded.len();
        let length = self.recorded.values().map(|v| v.len()).max().unwrap_or(0);
        (channels, length)
    }

    /// Given a fully-qualified label (node, output), return a `Sample` slice.
    pub fn get_output_by_label(&self, label: &str) -> &[Sample] {
        self.recorded
            .get(label)
            .unwrap_or_else(|| panic!("No such label: {label}"))
    }

    //--------------------------------------------------------------------------
    /// Write all recorded outputs to a multi-channel WAV file at `fp`.
    /// Each output in `output_names` becomes a distinct channel, in order.
    /// Samples are interleaved across channels for each frame.
    /// All channels are the same length after recording; any channel shorter than
    /// the maximum length is zero-padded for completeness.
    ///
    /// The `format` argument controls the bit depth and sample encoding:
    /// - `WavFormat::Float32` — 32-bit IEEE float (lossless for `f32` samples)
    /// - `WavFormat::Int16`   — 16-bit signed integer (samples scaled to `[-32768, 32767]`)
    /// - `WavFormat::Int24`   — 24-bit signed integer (samples scaled to `[-8388608, 8388607]`)
    pub fn to_wav(&self, fp: &Path, format: WavFormat) -> Result<(), hound::Error> {
        let channels = self.output_names.len() as u16;
        let (bits_per_sample, sample_format) = match format {
            WavFormat::Float32 => (32, hound::SampleFormat::Float),
            WavFormat::Int16 => (16, hound::SampleFormat::Int),
            WavFormat::Int24 => (24, hound::SampleFormat::Int),
        };
        let spec = hound::WavSpec {
            channels,
            sample_rate: self.sample_rate as u32,
            bits_per_sample,
            sample_format,
        };
        let mut writer = hound::WavWriter::create(fp, spec)?;
        let (_, length) = self.get_shape();
        for i in 0..length {
            for name in &self.output_names {
                let samples = &self.recorded[name];
                let s: f32 = if i < samples.len() { samples[i] } else { 0.0 };
                match format {
                    WavFormat::Float32 => writer.write_sample(s)?,
                    WavFormat::Int16 => {
                        let v = (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                        writer.write_sample(v)?;
                    }
                    WavFormat::Int24 => {
                        let v =
                            (s * 8388608.0).round().clamp(-8388608.0, 8388607.0) as i32;
                        writer.write_sample(v)?;
                    }
                }
            }
        }
        writer.finalize()?;
        Ok(())
    }

    /// Write all recorded outputs to a WAV stream without seeking.
    /// Identical channel layout and sample encoding to `to_wav`, but writes to
    /// any `Write` (including stdout). Safe because `Recorder` pre-computes all
    /// samples, so the RIFF/data chunk sizes are known before the first byte is written.
    pub fn to_wav_write<W: Write>(
        &self,
        mut w: W,
        format: WavFormat,
    ) -> std::io::Result<()> {
        let channels = self.output_names.len() as u16;
        let (bits_per_sample, audio_format): (u16, u16) = match format {
            WavFormat::Float32 => (32, 3), // IEEE_FLOAT
            WavFormat::Int16 => (16, 1),   // PCM
            WavFormat::Int24 => (24, 1),   // PCM
        };
        let sample_rate = self.sample_rate as u32;
        let bytes_per_sample = (bits_per_sample / 8) as u32;
        let block_align = channels as u32 * bytes_per_sample;
        let byte_rate = sample_rate * block_align;
        let (_, length) = self.get_shape();
        let data_size = length as u32 * channels as u32 * bytes_per_sample;
        let riff_size = 36u32 + data_size; // total file size minus the 8-byte RIFF preamble

        // RIFF header
        w.write_all(b"RIFF")?;
        w.write_all(&riff_size.to_le_bytes())?;
        w.write_all(b"WAVE")?;
        // fmt chunk (16 bytes of payload)
        w.write_all(b"fmt ")?;
        w.write_all(&16u32.to_le_bytes())?;
        w.write_all(&audio_format.to_le_bytes())?;
        w.write_all(&channels.to_le_bytes())?;
        w.write_all(&sample_rate.to_le_bytes())?;
        w.write_all(&byte_rate.to_le_bytes())?;
        w.write_all(&(block_align as u16).to_le_bytes())?;
        w.write_all(&bits_per_sample.to_le_bytes())?;
        // data chunk
        w.write_all(b"data")?;
        w.write_all(&data_size.to_le_bytes())?;

        for i in 0..length {
            for name in &self.output_names {
                let samples = &self.recorded[name];
                let s: f32 = if i < samples.len() { samples[i] } else { 0.0 };
                match format {
                    WavFormat::Float32 => w.write_all(&s.to_le_bytes())?,
                    WavFormat::Int16 => {
                        let v = (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
                        w.write_all(&v.to_le_bytes())?;
                    }
                    WavFormat::Int24 => {
                        let v =
                            (s * 8388608.0).round().clamp(-8388608.0, 8388607.0) as i32;
                        w.write_all(&v.to_le_bytes()[..3])?;
                    }
                }
            }
        }
        Ok(())
    }

    //--------------------------------------------------------------------------
    pub fn to_gnuplot(&self, fp: &Path) -> String {
        let (d, _samples) = self.get_shape();
        let base_height_per_lane = 100; // pixels per lane
        let width = 800;
        let height = d * base_height_per_lane;

        let mut script = String::new();

        script.push_str(&format!(
            "set terminal svg size {},{} background rgb '#12131E'\n",
            width, height
        ));
        script.push_str(&format!("set output '{}'\n\n", fp.display()));

        script.push_str(
            r#"# General appearance
set style line 11 lc rgb '#ffffff' lt 1
set tics out nomirror scale 0,0.001
set format y "%g"
unset key
set grid
set lmargin screen 0.15
set rmargin screen 0.98
set ytics font ",8"
set pointsize 0.5
unset xtics

# Color and style setup
do for [i=1:3] {
    set style line i lt 1 lw 1 pt 7 lc rgb '#5599ff'
}

# Multiplot setup
set multiplot
"#,
        );

        script.push_str(&format!("d = {d}\n"));
        script.push_str("margin = 0.04\n");
        script.push_str("height = 1.0 / d\n");
        script.push_str("pos = 1.0\n\n");
        script.push_str("label_x = 0.06\n");
        script.push_str("label_font = \",9\"\n\n");

        for (i, label) in self.output_names.iter().enumerate() {
            let values = self
                .recorded
                .get(label)
                .unwrap_or_else(|| panic!("expected label {label} not found"));
            let panel = i + 1;
            let block_label = label.replace(['.', '-', ' '], "_");

            // Data block
            script.push_str(&format!("${block_label} << EOD\n"));
            for v in values {
                script.push_str(&format!("{v}\n"));
            }
            script.push_str("EOD\n");

            script.push_str(&format!(
                r#"
# Panel {}
top = pos - margin * {}
bottom = pos - height + margin * 0.5
pos = pos - height
set tmargin screen top
set bmargin screen bottom
set label textcolor rgb '#c4c5bf'
set border lc rgb '#c4c5bf'
set grid lc rgb '#cccccc'
set label {} "{}" at screen label_x, screen (bottom + height / 2) center font label_font noenhanced
plot ${} using 1 with linespoints linestyle {}
"#,
                panel,
                if i == 0 { 1.0 } else { 0.5 },
                panel,
                label,
                block_label,
                (i % 3) + 1,
            ));
        }

        script.push_str("unset multiplot\n");
        for i in 1..=d {
            script.push_str(&format!("unset label {i}\n"));
        }

        script
    }

    pub fn to_gnuplot_fp(&self, fp: &str) -> std::io::Result<()> {
        let script = self.to_gnuplot(fp.as_ref());
        let mut file = NamedTempFile::new()?;
        write!(file, "{script}")?;
        let script_path = file.path();
        let status = Command::new("gnuplot").arg(script_path).status()?;

        if !status.success() {
            eprintln!("gnuplot failed with exit code: {:?}", status.code());
        }

        Ok(())
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::GenGraph;
    use crate::connect_many;
    use crate::register_many;
    use crate::{ModeRound, UGClock, UGEnvAR, UGRound, UGSine, UnitRate};

    #[test]
    fn test_recorder_a() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "clock" => UGClock::new(16.0, UnitRate::Samples),
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
        // plot_graph_to_image(&g, "/tmp/ampullator-old.png");
        let r1 = Recorder::from_samples(g, None, 10);
        assert_eq!(r1.get_shape(), (5, 10));
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();
    }

    #[test]
    fn test_recorder_b() {
        let mut g = GenGraph::new(8.0, 16);
        register_many![g,
            "clock" => UGClock::new(16.0, UnitRate::Samples),
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

        let output_labels = Some(vec!["round.out".to_string()]);
        let r1 = Recorder::from_samples(g, output_labels, 120);
        assert_eq!(r1.get_shape(), (1, 120));
    }

    #[test]
    fn test_recorder_from_duration() {
        let mut g = GenGraph::new(10.0, 8);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
        ];
        connect_many![g, "fq.out" -> "osc.freq"];

        // 1.0 second at 10 Hz sample rate => 10 samples
        // "fq" has 1 output, "osc" (UGSine) has 2 outputs (wave, trigger) → 3 channels
        let r1 = Recorder::from_duration(g, None, 1.0);
        assert_eq!(r1.get_shape(), (3, 10));
    }

    #[test]
    fn test_recorder_output_names_ordered_from_labels() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "clock" => UGClock::new(16.0, UnitRate::Samples),
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

        // Provide a specific, reversed order for output_labels
        let labels = vec!["round.out".to_string(), "clock.out".to_string()];
        let r1 = Recorder::from_samples(g, Some(labels.clone()), 16);
        // output_names must reflect the order we passed in
        assert_eq!(r1.output_names, labels);
        assert_eq!(r1.get_shape(), (2, 16));
    }

    #[test]
    fn test_recorder_to_wav_single_channel() {
        use tempfile::NamedTempFile;

        let mut g = GenGraph::new(10.0, 8);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
            "round" => UGRound::new(4, ModeRound::Round),
        ];
        connect_many![g,
            "fq.out" -> "osc.freq",
            "osc.wave" -> "round.in"
        ];

        let labels = Some(vec!["round.out".to_string()]);
        let r1 = Recorder::from_samples(g, labels, 10);

        let tmp = NamedTempFile::new().unwrap();
        r1.to_wav(tmp.path(), WavFormat::Float32).unwrap();

        // Re-open and verify header
        let mut reader = hound::WavReader::open(tmp.path()).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 10);
        assert_eq!(spec.bits_per_sample, 32);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);

        // Verify sample count and values
        let written: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(written.len(), 10);

        let expected = r1.get_output_by_label("round.out");
        for (a, b) in written.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 1e-6, "WAV sample mismatch: {a} != {b}");
        }
    }

    #[test]
    fn test_recorder_to_wav_multi_channel() {
        use tempfile::NamedTempFile;

        let mut g = GenGraph::new(10.0, 8);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
            "round" => UGRound::new(4, ModeRound::Round),
        ];
        connect_many![g,
            "fq.out" -> "osc.freq",
            "osc.wave" -> "round.in"
        ];

        // Record two channels in a specific order
        let labels = Some(vec!["round.out".to_string(), "osc.wave".to_string()]);
        let r1 = Recorder::from_samples(g, labels, 10);

        let tmp = NamedTempFile::new().unwrap();
        r1.to_wav(tmp.path(), WavFormat::Float32).unwrap();

        let mut reader = hound::WavReader::open(tmp.path()).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 2);
        assert_eq!(spec.sample_rate, 10);
        assert_eq!(spec.bits_per_sample, 32);
        assert_eq!(spec.sample_format, hound::SampleFormat::Float);

        // Samples are interleaved: [ch0[0], ch1[0], ch0[1], ch1[1], ...]
        let written: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(written.len(), 20); // 10 frames × 2 channels

        let ch0 = r1.get_output_by_label("round.out");
        let ch1 = r1.get_output_by_label("osc.wave");
        for i in 0..10 {
            assert!(
                (written[i * 2] - ch0[i]).abs() < 1e-6,
                "ch0 mismatch at frame {i}: {} != {}",
                written[i * 2],
                ch0[i]
            );
            assert!(
                (written[i * 2 + 1] - ch1[i]).abs() < 1e-6,
                "ch1 mismatch at frame {i}: {} != {}",
                written[i * 2 + 1],
                ch1[i]
            );
        }
    }

    #[test]
    fn test_recorder_to_wav_int16() {
        use tempfile::NamedTempFile;

        let mut g = GenGraph::new(10.0, 8);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
            "round" => UGRound::new(4, ModeRound::Round),
        ];
        connect_many![g,
            "fq.out" -> "osc.freq",
            "osc.wave" -> "round.in"
        ];

        let labels = Some(vec!["round.out".to_string()]);
        let r1 = Recorder::from_samples(g, labels, 10);

        let tmp = NamedTempFile::new().unwrap();
        r1.to_wav(tmp.path(), WavFormat::Int16).unwrap();

        let mut reader = hound::WavReader::open(tmp.path()).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 10);
        assert_eq!(spec.bits_per_sample, 16);
        assert_eq!(spec.sample_format, hound::SampleFormat::Int);

        let written: Vec<i16> = reader.samples::<i16>().map(|s| s.unwrap()).collect();
        assert_eq!(written.len(), 10);

        let expected = r1.get_output_by_label("round.out");
        for (i, (got, src)) in written.iter().zip(expected.iter()).enumerate() {
            let want = (*src * 32768.0).round().clamp(-32768.0, 32767.0) as i16;
            assert_eq!(
                *got, want,
                "Int16 sample mismatch at frame {i}: {got} != {want}"
            );
        }
    }

    #[test]
    fn test_recorder_to_wav_int24() {
        use tempfile::NamedTempFile;

        let mut g = GenGraph::new(10.0, 8);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
            "round" => UGRound::new(4, ModeRound::Round),
        ];
        connect_many![g,
            "fq.out" -> "osc.freq",
            "osc.wave" -> "round.in"
        ];

        let labels = Some(vec!["round.out".to_string()]);
        let r1 = Recorder::from_samples(g, labels, 10);

        let tmp = NamedTempFile::new().unwrap();
        r1.to_wav(tmp.path(), WavFormat::Int24).unwrap();

        let mut reader = hound::WavReader::open(tmp.path()).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 1);
        assert_eq!(spec.sample_rate, 10);
        assert_eq!(spec.bits_per_sample, 24);
        assert_eq!(spec.sample_format, hound::SampleFormat::Int);

        // hound reads 24-bit samples as i32
        let written: Vec<i32> = reader.samples::<i32>().map(|s| s.unwrap()).collect();
        assert_eq!(written.len(), 10);

        let expected = r1.get_output_by_label("round.out");
        for (i, (got, src)) in written.iter().zip(expected.iter()).enumerate() {
            let want = (*src * 8388608.0).round().clamp(-8388608.0, 8388607.0) as i32;
            assert_eq!(
                *got, want,
                "Int24 sample mismatch at frame {i}: {got} != {want}"
            );
        }
    }

    #[test]
    fn test_to_wav_write_round_trips() {
        // Verify that to_wav_write produces valid WAV data that decodes to the
        // correct samples. hound's WavReader is used to decode the in-memory
        // stream, confirming the header and sample data are both correct.
        // (to_wav uses hound's WAVE_FORMAT_EXTENSIBLE header for Float32, which
        // differs from the plain IEEE_FLOAT header we write — both are valid.)
        for format in [WavFormat::Float32, WavFormat::Int16, WavFormat::Int24] {
            let mut g = GenGraph::new(10.0, 8);
            register_many![g,
                "fq" => 0.5,
                "osc" => UGSine::new(),
                "round" => UGRound::new(4, ModeRound::Round),
            ];
            connect_many![g,
                "fq.out" -> "osc.freq",
                "osc.wave" -> "round.in"
            ];
            let labels = Some(vec!["round.out".to_string()]);
            let r = Recorder::from_samples(g, labels, 10);

            let mut buf = Vec::new();
            r.to_wav_write(&mut buf, format).unwrap();

            let cursor = std::io::Cursor::new(buf);
            let mut reader = hound::WavReader::new(cursor)
                .unwrap_or_else(|e| panic!("hound rejected {format:?} stream: {e}"));
            let spec = reader.spec();
            assert_eq!(spec.channels, 1);
            assert_eq!(spec.sample_rate, 10);

            let expected = r.get_output_by_label("round.out");
            let (decoded, tol): (Vec<f32>, f32) = match format {
                WavFormat::Float32 => {
                    (reader.samples::<f32>().map(|s| s.unwrap()).collect(), 1e-6)
                }
                WavFormat::Int16 => (
                    reader
                        .samples::<i16>()
                        .map(|s| s.unwrap() as f32 / 32768.0)
                        .collect(),
                    1.0 / 32768.0,
                ),
                WavFormat::Int24 => (
                    reader
                        .samples::<i32>()
                        .map(|s| s.unwrap() as f32 / 8388608.0)
                        .collect(),
                    1.0 / 8388608.0,
                ),
            };
            assert_eq!(decoded.len(), expected.len());
            for (i, (got, src)) in decoded.iter().zip(expected.iter()).enumerate() {
                assert!(
                    (got - src).abs() <= tol,
                    "{format:?} sample {i}: decoded {got} != expected {src}"
                );
            }
        }
    }

    #[test]
    fn test_recorder_c() {
        let mut g = GenGraph::new(10.0, 16);
        register_many![g,
            "fq" => 0.5,
            "osc" => UGSine::new(),
            "round" => UGRound::new(4, ModeRound::Round),
        ];

        connect_many![g,
            "fq.out" -> "osc.freq",
            "osc.wave" -> "round.in"
        ];

        let r1 = Recorder::from_samples(g, None, 120);
        assert_eq!(r1.get_shape(), (4, 120));
        assert_eq!(
            r1.get_output_by_label("round.out"),
            vec![
                0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809, 0.5878, 0.309, -0.0,
                -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511, -0.809, -0.5878, -0.309,
                0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809, 0.5878, 0.309,
                -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511, -0.809, -0.5878,
                -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809, 0.5878,
                0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511, -0.809,
                -0.5878, -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511, 0.809,
                0.5878, 0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0, -0.9511,
                -0.809, -0.5878, -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0, 0.9511,
                0.809, 0.5878, 0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511, -1.0,
                -0.9511, -0.809, -0.5878, -0.309, 0.0, 0.309, 0.5878, 0.809, 0.9511, 1.0,
                0.9511, 0.809, 0.5878, 0.309, -0.0, -0.309, -0.5878, -0.809, -0.9511,
                -1.0, -0.9511, -0.809, -0.5878, -0.309, 0.0
            ]
        );
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();
    }

    // #[test]
    // fn test_recorder_adhoc() {

    //     let fp = Path::new("/tmp/test.wav");

    //     let mut g = GenGraph::new(44100.0, 32);
    //     register_many![g,
    //         "fq" => 220,
    //         "amp" => 0.2,
    //         "osc" => UGSine::new(),
    //         "mult" => UGMult::new(2),
    //     ];
    //     connect_many![g,
    //         "fq.out" -> "osc.freq",
    //         "osc.wave" -> "mult.in1",
    //         "amp.out" -> "mult.in2"
    //     ];

    //     let labels = Some(vec!["mult.out".to_string()]);
    //     let r1 = Recorder::from_duration(g, labels, 5.0);

    //     r1.to_wav(fp, WavFormat::Int16).unwrap();
    //     println!("wrote: {:?}", fp);
    // }
}
