use serde::Deserialize;

use crate::GenGraph;
use crate::ModeRound;
use crate::Recorder;
use crate::ugen_core::UGen;
use crate::ugen_core::{
    LfoWave, UGAsHz, UGCeil, UGClock, UGConst, UGFade, UGFloor, UGLfo, UGMixLinear,
    UGMult, UGPan, UGRound, UGSine, UGSum, UGTrigger, UGWhite,
};
use crate::ugen_drum::{UGBassDrum, UGHighHat, UGSnareDrum};
use crate::ugen_env::{UGEnvAR, UGEnvBreakPoint};
use crate::ugen_filter::{
    UGHighPass, UGHighPassQ, UGLowPass, UGLowPassQ, UGParametric, UGParametricConst,
};
use crate::ugen_reverb::UGReverb;
use crate::ugen_rhythm::UGPulseSelect;
use crate::ugen_select::{ModeSelect, UGSelect};
use crate::util::Sample;
use crate::util::UnitRate;
use std::collections::HashMap;
use std::path::Path;

// The UGFacade provides enum-based deserialization of JSON encodings of UGen definition and intialization parameters.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, tag = "0", content = "1")]
pub enum UGFacade {
    AsHz {
        #[serde(default = "UGFacade::default_unit_rate_hz")]
        mode: UnitRate,
    },
    Ceil {},
    Clock {
        value: Sample,
        mode: UnitRate,
    },
    Const {
        value: Sample,
    },
    EnvBreakPoint {
        duration_values: Vec<Sample>,
        duration_mode: ModeSelect,
        level_values: Vec<Sample>,
        level_mode: ModeSelect,
        seed: Option<u64>,
    },
    EnvAR {},
    Fade {
        #[serde(default = "UGFacade::default_channels")]
        channels: usize,
        #[serde(default = "UGFacade::default_level")]
        level: f64,
    },
    Floor {},
    Lfo {
        wave: LfoWave,
        #[serde(default = "UGFacade::default_lfo_rate")]
        rate: Sample,
        #[serde(default = "UGFacade::default_unit_rate_hz")]
        mode: UnitRate,
        #[serde(default = "UGFacade::default_duty")]
        duty: Sample,
        #[serde(default = "UGFacade::default_lfo_min")]
        min: Sample,
        #[serde(default = "UGFacade::default_lfo_max")]
        max: Sample,
    },
    HighPass {
        #[serde(default = "UGFacade::default_roll_off_db")]
        roll_off_db: f32,
    },
    HighPassQ {
        #[serde(default = "UGFacade::default_roll_off_db")]
        roll_off_db: f32,
    },
    LowPass {
        #[serde(default = "UGFacade::default_roll_off_db")]
        roll_off_db: f32,
    },
    LowPassQ {
        #[serde(default = "UGFacade::default_roll_off_db")]
        roll_off_db: f32,
    },
    Parametric {},
    ParametricConst {
        gain: f32,
        bw: f32,
        freq: f32,
    },
    Pan {
        output_count: Option<usize>,
    },
    Mult {
        #[serde(default = "UGFacade::default_input_count")]
        input_count: usize,
    },
    MixLinear {
        #[serde(default = "UGFacade::default_mix_input_count")]
        input_count: usize,
        #[serde(default = "UGFacade::default_output_count")]
        output_count: usize,
    },
    PulseSelect {
        duration_values: Vec<Sample>,
        duration_mode: ModeSelect,
        seed: Option<u64>,
    },
    Round {
        #[serde(default = "UGFacade::default_round_places")]
        places: i32,
        #[serde(default = "UGFacade::default_mode_round")]
        mode: ModeRound,
    },
    Reverb {},
    Select {
        values: Vec<f32>,
        mode: ModeSelect,
        seed: Option<u64>,
    },
    Sine {},
    BassDrum {},
    HighHat {
        seed: Option<u64>,
    },
    SnareDrum {
        seed: Option<u64>,
    },
    Sum {
        #[serde(default = "UGFacade::default_input_count")]
        input_count: usize,
    },
    Trigger {},
    White {
        seed: Option<u64>,
    },
}

#[allow(unused)]
impl UGFacade {
    pub fn to_ugen(&self) -> Box<dyn UGen> {
        match self {
            UGFacade::Const { value } => Box::new(UGConst::new(*value)),
            UGFacade::Clock { value, mode } => Box::new(UGClock::new(*value, *mode)),
            UGFacade::Select { values, mode, seed } => {
                Box::new(UGSelect::new(values.clone(), *mode, *seed))
            }
            UGFacade::Round { places, mode } => Box::new(UGRound::new(*places, *mode)),
            UGFacade::Reverb {} => Box::new(UGReverb::new()),
            UGFacade::Sum { input_count } => Box::new(UGSum::new(*input_count)),
            UGFacade::White { seed } => Box::new(UGWhite::new(*seed)),
            UGFacade::AsHz { mode } => Box::new(UGAsHz::new(*mode)),
            UGFacade::Floor {} => Box::new(UGFloor::new()),
            UGFacade::Ceil {} => Box::new(UGCeil::new()),
            UGFacade::Mult { input_count } => Box::new(UGMult::new(*input_count)),
            UGFacade::MixLinear {
                input_count,
                output_count,
            } => Box::new(UGMixLinear::new(*input_count, *output_count)),
            UGFacade::Sine {} => Box::new(UGSine::new()),
            UGFacade::Lfo {
                wave,
                rate,
                mode,
                duty,
                min,
                max,
            } => Box::new(UGLfo::new(*wave, *rate, *mode, *duty, *min, *max)),
            UGFacade::BassDrum {} => Box::new(UGBassDrum::new()),
            UGFacade::HighHat { seed } => Box::new(UGHighHat::new(*seed)),
            UGFacade::SnareDrum { seed } => Box::new(UGSnareDrum::new_seeded(*seed)),
            UGFacade::Trigger {} => Box::new(UGTrigger::new()),
            UGFacade::HighPass { roll_off_db } => Box::new(UGHighPass::new(*roll_off_db)),
            UGFacade::HighPassQ { roll_off_db } => {
                Box::new(UGHighPassQ::new(*roll_off_db))
            }
            UGFacade::LowPass { roll_off_db } => Box::new(UGLowPass::new(*roll_off_db)),
            UGFacade::LowPassQ { roll_off_db } => Box::new(UGLowPassQ::new(*roll_off_db)),
            UGFacade::Parametric {} => Box::new(UGParametric::new()),
            UGFacade::ParametricConst { gain, bw, freq } => {
                Box::new(UGParametricConst::new(*gain, *bw, *freq))
            }
            UGFacade::Pan { output_count } => {
                Box::new(UGPan::new(output_count.unwrap_or(2)))
            }
            UGFacade::EnvBreakPoint {
                duration_values,
                duration_mode,
                level_values,
                level_mode,
                seed,
            } => Box::new(UGEnvBreakPoint::new(
                duration_values.clone(),
                *duration_mode,
                level_values.clone(),
                *level_mode,
                *seed,
            )),
            UGFacade::EnvAR {} => Box::new(UGEnvAR::new()),
            UGFacade::Fade { channels, level } => {
                Box::new(UGFade::new(*channels, *level as f32))
            }
            UGFacade::PulseSelect {
                duration_values,
                duration_mode,
                seed,
            } => Box::new(UGPulseSelect::new(
                duration_values.clone(),
                *duration_mode,
                *seed,
            )),
        }
    }

    /// Return `true` if `name` is a recognized UGFacade variant name.
    ///
    /// Used by the Chain DSL parser to distinguish UGen type names from
    /// user-defined node name references. The check is driven by serde:
    /// `["name", {}]` is attempted; an "unknown variant" error means the
    /// name is not a valid variant, while any other outcome (success or a
    /// different deserialization error such as a missing required field)
    /// confirms the name is a valid variant.
    pub fn is_variant_name(name: &str) -> bool {
        let probe = serde_json::Value::Array(vec![
            serde_json::Value::String(name.to_string()),
            serde_json::Value::Null,
        ]);
        match serde_json::from_value::<UGFacade>(probe) {
            Ok(_) => true,
            Err(e) => !e.to_string().contains("unknown variant"),
        }
    }

    // Serde default helpers -- used by `#[serde(default = "...")]` on UGFacade fields.

    fn default_roll_off_db() -> f32 {
        6.0
    }

    fn default_unit_rate_hz() -> UnitRate {
        UnitRate::Hz
    }

    fn default_round_places() -> i32 {
        0
    }

    fn default_mode_round() -> ModeRound {
        ModeRound::Round
    }

    fn default_input_count() -> usize {
        2
    }

    fn default_channels() -> usize {
        1
    }

    fn default_level() -> f64 {
        1.0
    }

    fn default_mix_input_count() -> usize {
        2
    }

    fn default_output_count() -> usize {
        2
    }

    fn default_lfo_rate() -> Sample {
        1.0
    }

    fn default_duty() -> Sample {
        0.5
    }

    fn default_lfo_min() -> Sample {
        0.0
    }

    fn default_lfo_max() -> Sample {
        1.0
    }
}

//------------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
#[serde(untagged)]
#[allow(unused)]
pub enum Facade {
    Short(f32),     // concise numeric constant: "step": 1
    Full(UGFacade), // ["Clock", { ... }] or ["Round", { ... }]
}

#[allow(unused)]
impl Facade {
    pub fn to_ugen(&self) -> Box<dyn UGen> {
        match self {
            Facade::Short(f) => Box::new(UGConst::new(*f)),
            Facade::Full(facade) => facade.to_ugen(),
        }
    }
}

//------------------------------------------------------------------------------
// NOTE: we do not need these with the methods below

// pub fn register_many(graph: &mut GenGraph, j: &str) {
//     let defs: HashMap<String, Facade> = serde_json::from_str(j).unwrap();
//     for (name, def) in defs {
//         graph.add_node(name, def.to_ugen());
//     }
// }

// /// Connects nodes in a GenGraph using a JSON string of `"src": "dst"` mappings.
// pub fn connect_many(graph: &mut GenGraph, j: &str) {
//     let pairs: HashMap<String, String> =
//         serde_json::from_str(j).expect("Failed to parse connection JSON");
//     for (src, dst) in pairs {
//         graph.connect(&src, &dst);
//     }
// }

//------------------------------------------------------------------------------

#[allow(unused)]
#[derive(Deserialize, Debug)]
pub(crate) struct GraphFacade {
    title: Option<String>,
    label: Option<String>,
    chain: Option<String>,
    #[serde(default = "GraphFacade::default_sample_rate")]
    sample_rate: f32,
    #[serde(default = "GraphFacade::default_buffer_size")]
    buffer_size: usize,
    #[serde(default = "GraphFacade::default_total_samples")]
    total_samples: usize,
    #[serde(default)]
    register: HashMap<String, Facade>,
    #[serde(default)]
    connect: Vec<(String, String)>,
}

#[allow(unused)]
impl GraphFacade {
    fn default_sample_rate() -> f32 {
        100.0
    }
    fn default_buffer_size() -> usize {
        8
    }
    fn default_total_samples() -> usize {
        100
    }

    pub fn from_json(json: &str) -> Result<Self, String> {
        let mut facade: Self = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse JSON: {e}"))?;
        if let Some(ref chain) = facade.chain {
            if !facade.register.is_empty() || !facade.connect.is_empty() {
                return Err(
                    "Cannot specify both 'chain' and 'register'/'connect'".to_string()
                );
            }
            let (register, connect) = crate::chain::parse_chain(chain)?;
            facade.register = register;
            facade.connect = connect;
        }
        Ok(facade)
    }

    /// Construct a `GraphFacade` by parsing a Chain DSL string.
    ///
    /// The resulting `register` and `connect` containers are equivalent to
    /// those you would get from the JSON form and can be used with
    /// [`register_and_connect`] to build a [`GenGraph`].
    pub fn from_chain(chain: &str) -> Result<Self, String> {
        let (register, connect) = crate::chain::parse_chain(chain)?;
        Ok(Self {
            title: None,
            label: None,
            chain: Some(chain.to_string()),
            sample_rate: Self::default_sample_rate(),
            buffer_size: Self::default_buffer_size(),
            total_samples: Self::default_total_samples(),
            register,
            connect,
        })
    }

    pub fn register_and_connect(&self, graph: &mut GenGraph) -> Result<(), String> {
        // Register all nodes in sorted order for deterministic output
        let mut keys: Vec<_> = self.register.keys().collect();
        keys.sort();
        for name in keys {
            let facade = &self.register[name];
            eprintln!("register: {:?}", name);
            graph.add_node(name, facade.to_ugen());
        }
        // Connect nodes
        for (src, dst) in &self.connect {
            eprintln!("connect: {:?} -> {:?}", src, dst);
            graph.connect(src, dst);
        }
        Ok(())
    }

    /// Based on this GraphFacade, create a Graph and render both a graph figure and a time-domain plot figure.
    fn to_rendered_figures(&self, dir: &Path) -> Result<(String, String), String> {
        let mut g = GenGraph::new(self.sample_rate, self.buffer_size);
        let _ = self.register_and_connect(&mut g);

        let name = self.label.clone().unwrap_or_else(|| "graph".to_string());

        let fn_graph = format!("{name}_graph.svg");
        let fn_time_domain = format!("{name}_time-domain.svg");

        let fp_graph = dir.join(&fn_graph);
        let _ = g.to_dot_fp(&fp_graph);

        let fp_time_domain = dir.join(&fn_time_domain);
        let r1 = Recorder::from_samples(g, None, self.total_samples);
        r1.to_gnuplot_fp(fp_time_domain.to_str().unwrap()).unwrap();

        Ok((fn_graph, fn_time_domain))
    }
}

#[allow(unused)]
const GITHUB_BASE_URL: &str =
    "https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/";

//------------------------------------------------------------------------------

/// One construction argument for a Chain DSL UGen.
struct FacadeArgDoc {
    name: &'static str,
    type_hint: &'static str,
    /// `None` means the argument is required (no default).
    default: Option<&'static str>,
}

impl FacadeArgDoc {
    fn required(name: &'static str, type_hint: &'static str) -> Self {
        Self {
            name,
            type_hint,
            default: None,
        }
    }
    fn optional(
        name: &'static str,
        type_hint: &'static str,
        default: &'static str,
    ) -> Self {
        Self {
            name,
            type_hint,
            default: Some(default),
        }
    }
}

/// Format a float default value for display: integers without decimal point.
fn fmt_sample(v: f32) -> String {
    if v.fract() == 0.0 && v.abs() < 1.0e6 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Generate a markdown section documenting all Chain DSL UGen interfaces.
///
/// Each entry lists the UGen name, its construction arguments (with defaults),
/// signal inputs with their default values, and signal outputs. Content is
/// derived directly from the [`UGFacade`] definitions and the [`UGen`] trait
/// implementations so it can be regenerated to reflect the current interface.
fn chain_ugen_reference_markdown() -> String {
    use crate::ugen_core::{LfoWave, ModeRound};
    use crate::ugen_drum::{UGBassDrum, UGHighHat, UGSnareDrum};
    use crate::ugen_env::{UGEnvAR, UGEnvBreakPoint};
    use crate::ugen_filter::{
        UGHighPass, UGHighPassQ, UGLowPass, UGLowPassQ, UGParametric, UGParametricConst,
    };
    use crate::ugen_reverb::UGReverb;
    use crate::ugen_rhythm::UGPulseSelect;
    use crate::ugen_select::{ModeSelect, UGSelect};

    const UNIT_RATE: &str = "`Hz` \\| `Bpm` \\| `Samples` \\| `Midi` \\| `Seconds`";
    const MODE_SELECT: &str = "`Cycle` \\| `Random` \\| `Shuffle` \\| `Walk`";
    const MODE_ROUND: &str = "`Round` \\| `Floor` \\| `Ceil`";
    const LFO_WAVE: &str = "`Sine` \\| `Triangle` \\| `Square`";

    // (facade_name, construction_args, representative_ugen_instance)
    let variants: Vec<(&str, Vec<FacadeArgDoc>, Box<dyn UGen>)> = vec![
        (
            "AsHz",
            vec![FacadeArgDoc::optional("mode", UNIT_RATE, "Hz")],
            Box::new(UGAsHz::new(UnitRate::Hz)),
        ),
        ("BassDrum", vec![], Box::new(UGBassDrum::new())),
        ("Ceil", vec![], Box::new(UGCeil::new())),
        (
            "Clock",
            vec![
                FacadeArgDoc::required("value", "number"),
                FacadeArgDoc::required("mode", UNIT_RATE),
            ],
            Box::new(UGClock::new(120.0, UnitRate::Bpm)),
        ),
        (
            "Const",
            vec![FacadeArgDoc::required("value", "number")],
            Box::new(UGConst::new(0.0)),
        ),
        ("EnvAR", vec![], Box::new(UGEnvAR::new())),
        (
            "EnvBreakPoint",
            vec![
                FacadeArgDoc::required("duration_values", "[number, ...]"),
                FacadeArgDoc::required("duration_mode", MODE_SELECT),
                FacadeArgDoc::required("level_values", "[number, ...]"),
                FacadeArgDoc::required("level_mode", MODE_SELECT),
                FacadeArgDoc::optional("seed", "integer", "none"),
            ],
            Box::new(UGEnvBreakPoint::new(
                vec![1.0],
                ModeSelect::Cycle,
                vec![1.0],
                ModeSelect::Cycle,
                None,
            )),
        ),
        (
            "Fade",
            vec![
                FacadeArgDoc::optional("channels", "integer", "1"),
                FacadeArgDoc::optional("level", "number", "1.0"),
            ],
            Box::new(UGFade::new(1, 1.0)),
        ),
        ("Floor", vec![], Box::new(UGFloor::new())),
        (
            "HighHat",
            vec![FacadeArgDoc::optional("seed", "integer", "none")],
            Box::new(UGHighHat::new(None)),
        ),
        (
            "HighPass",
            vec![FacadeArgDoc::optional("roll_off_db", "number", "6.0")],
            Box::new(UGHighPass::new(6.0)),
        ),
        (
            "HighPassQ",
            vec![FacadeArgDoc::optional("roll_off_db", "number", "6.0")],
            Box::new(UGHighPassQ::new(6.0)),
        ),
        (
            "Lfo",
            vec![
                FacadeArgDoc::required("wave", LFO_WAVE),
                FacadeArgDoc::optional("rate", "number", "1.0"),
                FacadeArgDoc::optional("mode", UNIT_RATE, "Hz"),
                FacadeArgDoc::optional("duty", "number", "0.5"),
                FacadeArgDoc::optional("min", "number", "0.0"),
                FacadeArgDoc::optional("max", "number", "1.0"),
            ],
            Box::new(UGLfo::new(LfoWave::Sine, 1.0, UnitRate::Hz, 0.5, 0.0, 1.0)),
        ),
        (
            "LowPass",
            vec![FacadeArgDoc::optional("roll_off_db", "number", "6.0")],
            Box::new(UGLowPass::new(6.0)),
        ),
        (
            "LowPassQ",
            vec![FacadeArgDoc::optional("roll_off_db", "number", "6.0")],
            Box::new(UGLowPassQ::new(6.0)),
        ),
        (
            "MixLinear",
            vec![
                FacadeArgDoc::optional("input_count", "integer", "2"),
                FacadeArgDoc::optional("output_count", "integer", "2"),
            ],
            Box::new(UGMixLinear::new(2, 2)),
        ),
        (
            "Mult",
            vec![FacadeArgDoc::optional("input_count", "integer", "2")],
            Box::new(UGMult::new(2)),
        ),
        (
            "Pan",
            vec![FacadeArgDoc::optional("output_count", "integer", "2")],
            Box::new(UGPan::new(2)),
        ),
        ("Parametric", vec![], Box::new(UGParametric::new())),
        (
            "ParametricConst",
            vec![
                FacadeArgDoc::required("gain", "number"),
                FacadeArgDoc::required("bw", "number"),
                FacadeArgDoc::required("freq", "number"),
            ],
            Box::new(UGParametricConst::new(0.0, 0.333, 1000.0)),
        ),
        (
            "PulseSelect",
            vec![
                FacadeArgDoc::required("duration_values", "[number, ...]"),
                FacadeArgDoc::required("duration_mode", MODE_SELECT),
                FacadeArgDoc::optional("seed", "integer", "none"),
            ],
            Box::new(UGPulseSelect::new(vec![1.0], ModeSelect::Cycle, None)),
        ),
        ("Reverb", vec![], Box::new(UGReverb::new())),
        (
            "Round",
            vec![
                FacadeArgDoc::optional("places", "integer", "0"),
                FacadeArgDoc::optional("mode", MODE_ROUND, "Round"),
            ],
            Box::new(UGRound::new(0, ModeRound::Round)),
        ),
        (
            "Select",
            vec![
                FacadeArgDoc::required("values", "[number, ...]"),
                FacadeArgDoc::required("mode", MODE_SELECT),
                FacadeArgDoc::optional("seed", "integer", "none"),
            ],
            Box::new(UGSelect::new(vec![0.0], ModeSelect::Cycle, None)),
        ),
        ("Sine", vec![], Box::new(UGSine::new())),
        (
            "SnareDrum",
            vec![FacadeArgDoc::optional("seed", "integer", "none")],
            Box::new(UGSnareDrum::new_seeded(None)),
        ),
        (
            "Sum",
            vec![FacadeArgDoc::optional("input_count", "integer", "2")],
            Box::new(UGSum::new(2)),
        ),
        ("Trigger", vec![], Box::new(UGTrigger::new())),
        (
            "White",
            vec![FacadeArgDoc::optional("seed", "integer", "none")],
            Box::new(UGWhite::new(None)),
        ),
    ];

    let mut md: Vec<String> = Vec::new();
    md.push("## Chain DSL UGen Reference".to_string());
    md.push("".to_string());
    md.push(
        "The following UGens are available in the Chain DSL. \
         Each entry lists construction arguments (with defaults), \
         signal inputs (with default values), and signal outputs."
            .to_string(),
    );
    md.push("".to_string());

    for (name, args, ugen) in &variants {
        md.push(format!("### {name}"));
        md.push("".to_string());

        if !args.is_empty() {
            md.push("**Construction args:**".to_string());
            md.push("".to_string());
            md.push("| Arg | Type | Default |".to_string());
            md.push("|-----|------|---------|".to_string());
            for arg in args {
                let default = arg
                    .default
                    .map_or("*required*".to_string(), |d| format!("`{d}`"));
                md.push(format!(
                    "| `{}` | {} | {} |",
                    arg.name, arg.type_hint, default
                ));
            }
            md.push("".to_string());
        }

        let inputs = ugen.input_names();
        if !inputs.is_empty() {
            md.push("**Inputs:**".to_string());
            md.push("".to_string());
            md.push("| Input | Default |".to_string());
            md.push("|-------|---------|".to_string());
            for input in inputs {
                let default = ugen
                    .default_input(input)
                    .map_or("—".to_string(), |v| format!("`{}`", fmt_sample(v)));
                md.push(format!("| `{input}` | {default} |"));
            }
            md.push("".to_string());
        }

        let outputs = ugen.output_names();
        let outputs_str: Vec<String> = outputs.iter().map(|o| format!("`{o}`")).collect();
        md.push(format!("**Outputs:** {}", outputs_str.join(", ")));
        md.push("".to_string());
    }

    md.join("\n")
}

pub fn build_markdown_index(
    input_dir: &Path,
    output_dir: &Path,
    usage_path: &Path,
    readme_path: &Path,
    abs_paths: bool,
) -> Result<(), String> {
    let usage = std::fs::read_to_string(usage_path).map_err(|e| {
        format!("Failed to read usage file '{}': {e}", usage_path.display())
    })?;

    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    entries.push("## Examples".to_string());
    entries.push("".to_string());

    // Collect and sort JSON example files for deterministic output order.
    let mut paths: Vec<_> = std::fs::read_dir(input_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();

    for path in paths {
        println!("build_markdown_index: parsing: {:?}", path);

        let json_str = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())?
            .trim()
            .to_string();
        let parsed = GraphFacade::from_json(&json_str)?;

        let title = parsed.title.clone().unwrap_or("title".to_string());
        let label = parsed.label.clone().unwrap_or("label".to_string());

        let (fn_graph, fn_time_domain) = parsed.to_rendered_figures(output_dir)?;

        let img_url = |filename: &str| -> String {
            if abs_paths {
                format!("{}{}/{}", GITHUB_BASE_URL, output_dir.display(), filename)
            } else {
                filename.to_string()
            }
        };

        entries.push(format!("### {title}"));
        if let Some(ref chain) = parsed.chain {
            entries.push("```text".to_string());
            for (i, segment) in chain.split('|').enumerate() {
                let segment = segment.trim();
                if i == 0 {
                    entries.push(segment.to_string());
                } else {
                    entries.push(format!("| {segment}"));
                }
            }
            entries.push("```".to_string());
        } else {
            entries.push("```json".to_string());
            entries.push(json_str.clone());
            entries.push("```".to_string());
        }
        entries.push(format!("![{label}]({})", img_url(&fn_graph)));
        entries.push(format!("![{label}]({})", img_url(&fn_time_domain)));
        entries.push("".to_string()); // blank line for spacing
    }

    let ugen_ref = chain_ugen_reference_markdown();
    let readme = format!(
        "{}\n\n{}\n\n{}",
        usage.trim_end(),
        ugen_ref,
        entries.join("\n")
    );
    std::fs::write(readme_path, readme).map_err(|e| e.to_string())?;
    Ok(())
}

/// Build a [`GenGraph`] from a Chain DSL expression, using explicit runtime
/// graph settings.
pub fn graph_from_chain_expression(
    chain: &str,
    sample_rate: f32,
    buffer_size: usize,
) -> Result<GenGraph, String> {
    let facade = GraphFacade::from_chain(chain)?;
    let mut graph = GenGraph::new(sample_rate, buffer_size);
    facade.register_and_connect(&mut graph)?;
    Ok(graph)
}

/// Build a [`GenGraph`] from GraphFacade JSON content, using explicit runtime
/// graph settings.
pub fn graph_from_json_definition(
    json: &str,
    sample_rate: f32,
    buffer_size: usize,
) -> Result<GenGraph, String> {
    let facade = GraphFacade::from_json(json)?;
    let mut graph = GenGraph::new(sample_rate, buffer_size);
    facade.register_and_connect(&mut graph)?;
    Ok(graph)
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Recorder;
    use std::collections::HashMap;

    //--------------------------------------------------------------------------
    #[test]
    fn test_ug_facade_a() {
        let j = r#"
        {
          "clock": ["Clock", {"value": 2.0, "mode": "Samples" }]
        }"#;

        let defs: HashMap<String, UGFacade> = serde_json::from_str(j).unwrap();
        assert_eq!(defs.len(), 1);
    }

    #[test]
    fn test_ug_facade_b() {
        let json = r#"{
            "register" : {
                "c1": ["Const", {"value": 1.0 }],
                "c2": 4,
                "clock": ["Clock", { "value": 2.0, "mode": "Samples" }],
                "rounder": ["Round", { "places": 2, "mode": "Round" }]
            },
            "connect": []
        }
        "#;

        let gf = GraphFacade::from_json(json).unwrap();
        let mut g = GenGraph::new(44_100.0, 8);
        let _ = gf.register_and_connect(&mut g);
        assert_eq!(g.len(), 4);
    }

    #[test]
    fn test_ug_facade_c() {
        let json = r#"{
        "register": {
            "step": 1,
            "clock": ["Clock", { "value": 2.0, "mode": "Samples" }],
            "sel": ["Select", { "values": [10, 5, 15, 20], "mode": "Shuffle", "seed": 42 }]
        },
        "connect": [
          ["clock.out", "sel.trigger"],
          ["step.out", "sel.step"]
        ]
    }
        "#;
        let gf = GraphFacade::from_json(json).unwrap();
        let mut g = GenGraph::new(8.0, 8);
        let _ = gf.register_and_connect(&mut g);
        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![
                15.0, 15.0, 20.0, 20.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 5.0, 5.0, 20.0,
                20.0, 10.0, 10.0, 15.0, 15.0, 5.0, 5.0, 10.0, 10.0, 20.0, 20.0, 20.0,
                20.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 5.0, 5.0, 20.0, 20.0, 15.0, 15.0,
                10.0, 10.0, 15.0, 15.0, 5.0, 5.0, 10.0, 10.0, 20.0, 20.0, 10.0, 10.0,
                5.0, 5.0, 20.0, 20.0, 15.0, 15.0, 5.0, 5.0, 20.0, 20.0, 15.0, 15.0, 10.0,
                10.0, 10.0, 10.0, 15.0, 15.0, 20.0, 20.0, 5.0, 5.0, 15.0, 15.0, 20.0,
                20.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 10.0, 10.0, 20.0, 20.0, 5.0, 5.0,
                15.0, 15.0, 20.0, 20.0, 5.0, 5.0, 10.0, 10.0, 20.0, 20.0, 5.0, 5.0
            ]
        );
    }

    #[test]
    fn test_ug_facade_d() {
        let json = r#"
        {
            "register": {
                "step": 1,
                "clock": ["Clock", { "value": 2.0, "mode": "Samples" }],
                "sel": ["Select", { "values": [10, 5, 15, 20], "mode": "Walk", "seed": 42 }]
            },
            "connect": [
              ["clock.out", "sel.trigger"],
              ["step.out", "sel.step"]
            ]
        }
        "#;

        let mut g = GenGraph::new(8.0, 8);

        let gf: GraphFacade = serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse JSON: {e}"))
            .unwrap();
        let res = gf.register_and_connect(&mut g);
        assert!(res.is_ok(), "Failed to register/connect: {:?}", res);

        let r1 = Recorder::from_samples(g, None, 50);
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![
                15.0, 15.0, 5.0, 5.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 20.0, 20.0, 15.0,
                15.0, 5.0, 5.0, 15.0, 15.0, 20.0, 20.0, 15.0, 15.0, 5.0, 5.0, 15.0, 15.0,
                20.0, 20.0, 15.0, 15.0, 20.0, 20.0, 10.0, 10.0, 20.0, 20.0, 10.0, 10.0,
                20.0, 20.0, 15.0, 15.0, 20.0, 20.0, 15.0, 15.0, 20.0, 20.0, 10.0, 10.0
            ]
        );
    }

    #[test]
    fn test_ug_facade_as_hz() {
        // 4 samples at sr=8 → 2 Hz; trigger fires at samples 0 and 4
        let json = r#"{
            "register": {
                "c1": ["Const", {"value": 4.0}],
                "hz": ["AsHz", {"mode": "Samples"}],
                "trig": ["Trigger", {}]
            },
            "connect": [
                ["c1.out", "hz.in"],
                ["hz.out", "trig.freq"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("trig.out"),
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn test_ug_facade_floor() {
        let json = r#"{
            "register": {
                "c1": ["Const", {"value": 2.7}],
                "f1": ["Floor", {}]
            },
            "connect": [["c1.out", "f1.in"]]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("f1.out"),
            vec![2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]
        );
    }

    #[test]
    fn test_ug_facade_ceil() {
        let json = r#"{
            "register": {
                "c1": ["Const", {"value": 2.3}],
                "cg": ["Ceil", {}]
            },
            "connect": [["c1.out", "cg.in"]]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("cg.out"),
            vec![3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0]
        );
    }

    #[test]
    fn test_ug_facade_mult() {
        let json = r#"{
            "register": {
                "c1": ["Const", {"value": 3.0}],
                "c2": ["Const", {"value": 4.0}],
                "m1": ["Mult", {"input_count": 2}]
            },
            "connect": [
                ["c1.out", "m1.in1"],
                ["c2.out", "m1.in2"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("m1.out"),
            vec![12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0, 12.0]
        );
    }

    #[test]
    fn test_ug_facade_reverb() {
        let chain = "Const(value=0.25) => l | \
                     Const(value=-0.5) => r | \
                     Const(value=0.0) => m | \
                     Reverb() => rev | \
                     l ->:in_l rev | \
                     r ->:in_r rev | \
                     m ->:mix rev";
        let mut g = GenGraph::new(8.0, 8);
        let gf = GraphFacade::from_chain(chain).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("rev.out_l"),
            vec![0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25]
        );
        assert_eq!(
            g.get_output_by_label("rev.out_r"),
            vec![-0.5, -0.5, -0.5, -0.5, -0.5, -0.5, -0.5, -0.5]
        );
    }

    #[test]
    fn test_ug_facade_pan() {
        let json = r#"{
            "register": {
                "in": ["Const", {"value": 1.0}],
                "pan_pos": ["Const", {"value": 0.5}],
                "pan": ["Pan", {}],
                "l": ["Round", {"places": 3, "mode": "Round"}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["in.out", "pan.in"],
                ["pan_pos.out", "pan.pan"],
                ["pan.out1", "l.in"],
                ["pan.out2", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("l.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707, 0.707]
        );
    }

    #[test]
    fn test_ug_facade_sine() {
        let json = r#"{
            "register": {
                "freq": ["Const", {"value": 1.0}],
                "osc": ["Sine", {}],
                "r": ["Round", {"places": 1, "mode": "Round"}]
            },
            "connect": [
                ["freq.out", "osc.freq"],
                ["osc.wave", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![0.7, 1.0, 0.7, -0.0, -0.7, -1.0, -0.7, 0.0]
        );
    }

    #[test]
    fn test_ug_facade_trigger() {
        let json = r#"{
            "register": {
                "freq": 4.0,
                "trig": ["Trigger", {}]
            },
            "connect": [["freq.out", "trig.freq"]]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("trig.out"),
            vec![1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn test_ug_facade_low_pass() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "cutoff": 60.0,
                "lpf": ["LowPass", {"roll_off_db": 12.0}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "lpf.in"],
                ["cutoff.out", "lpf.cutoff"],
                ["lpf.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(2000.0, 16);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.036, 0.058, 0.07, 0.076, 0.077, 0.075, 0.071, 0.066, 0.06, 0.054,
                0.048, 0.043, 0.038, 0.033, 0.029, 0.025
            ]
        );
    }

    #[test]
    fn test_ug_facade_low_pass_q() {
        // resonance defaults to 0.0, so output matches LowPass
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "cutoff": 60.0,
                "lpfq": ["LowPassQ", {"roll_off_db": 12.0}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "lpfq.in"],
                ["cutoff.out", "lpfq.cutoff"],
                ["lpfq.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(2000.0, 16);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.036, 0.058, 0.07, 0.076, 0.077, 0.075, 0.071, 0.066, 0.06, 0.054,
                0.048, 0.043, 0.038, 0.033, 0.029, 0.025
            ]
        );
    }

    #[test]
    fn test_ug_facade_high_pass() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "cutoff": 60.0,
                "hpf": ["HighPass", {"roll_off_db": 12.0}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "hpf.in"],
                ["cutoff.out", "hpf.cutoff"],
                ["hpf.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(2000.0, 16);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.659, -0.248, -0.178, -0.126, -0.086, -0.058, -0.037, -0.021, -0.011,
                -0.003, 0.002, 0.005, 0.007, 0.008, 0.008, 0.008
            ]
        );
    }

    #[test]
    fn test_ug_facade_high_pass_q() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "cutoff": 60.0,
                "resonance": 0.5,
                "hpfq": ["HighPassQ", {"roll_off_db": 12.0}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "hpfq.in"],
                ["cutoff.out", "hpfq.cutoff"],
                ["resonance.out", "hpfq.resonance"],
                ["hpfq.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(2000.0, 16);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                0.659, -0.465, 0.057, -0.143, -0.032, -0.06, -0.031, -0.03, -0.018,
                -0.013, -0.008, -0.004, -0.001, 0.002, 0.004, 0.005
            ]
        );
    }

    #[test]
    fn test_ug_facade_env_break_point() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 2.0, "mode": "Samples"}],
                "env": ["EnvBreakPoint", {
                    "duration_values": [2.0, 4.0, 3.0, 2.0],
                    "duration_mode": "Cycle",
                    "level_values": [1.0, 0.2, 0.8, 0.5],
                    "level_mode": "Cycle",
                    "seed": 42
                }],
                "r": ["Round", {"places": 4, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "env.clock"],
                ["env.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        let r1 = Recorder::from_samples(g, None, 40);
        assert_eq!(
            r1.get_output_by_label("r.out"),
            vec![
                1.0, 1.0, 1.0, 1.0, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.8, 0.8,
                0.8, 0.8, 0.8, 0.8, 0.5, 0.5, 0.5, 0.5, 1.0, 1.0, 1.0, 1.0, 0.2, 0.2,
                0.2, 0.2, 0.2, 0.2, 0.2, 0.2, 0.8, 0.8, 0.8, 0.8, 0.8, 0.8
            ]
        );
    }

    #[test]
    fn test_ug_facade_env_ar() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "env": ["EnvAR", {}],
                "a": 4,
                "r": 8,
                "round": ["Round", {"places": 4, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "env.trigger"],
                ["a.out", "env.attack_dur"],
                ["r.out", "env.release_dur"],
                ["env.out", "round.in"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        let r1 = Recorder::from_samples(g, None, 40);
        assert_eq!(
            r1.get_output_by_label("round.out"),
            vec![
                0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 0.875, 0.75, 0.625, 0.5, 0.375, 0.25,
                0.125, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0,
                0.875, 0.75, 0.625, 0.5, 0.375, 0.25, 0.125, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0
            ]
        );
    }

    #[test]
    fn test_ug_facade_pulse_select() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 1.0, "mode": "Samples"}],
                "step": 1,
                "pulse": ["PulseSelect", {
                    "duration_values": [3.0, 1.0, 4.0, 2.0],
                    "duration_mode": "Cycle",
                    "seed": 42
                }]
            },
            "connect": [
                ["clock.out", "pulse.clock"],
                ["step.out", "pulse.step"]
            ]
        }"#;
        let mut g = GenGraph::new(8.0, 8);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
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
    fn test_ug_facade_bass_drum() {
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 32.0, "mode": "Samples"}],
                "kick": ["BassDrum", {}]
            },
            "connect": [
                ["clock.out", "kick.gate"]
            ]
        }"#;
        let mut g = GenGraph::new(44100.0, 32);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        let out = g.get_output_by_label("kick.out");
        assert!(
            out.iter().any(|s| s.abs() > 0.0),
            "bass drum facade should produce output when triggered"
        );
    }

    #[test]
    fn test_ug_facade_parametric() {
        // 6 dB boost at 60 Hz with 1/3-octave bw via JSON facade.
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "gain": 6.0,
                "bw": 0.333,
                "freq": 60.0,
                "pq": ["Parametric", {}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "pq.in"],
                ["gain.out", "pq.gain"],
                ["bw.out", "pq.bw"],
                ["freq.out", "pq.freq"],
                ["pq.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(2000.0, 16);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.015, 0.029, 0.027, 0.024, 0.02, 0.015, 0.01, 0.005, -0.0, -0.005,
                -0.01, -0.014, -0.018, -0.021, -0.023, -0.024,
            ]
        );
    }

    #[test]
    fn test_ug_facade_parametric_const() {
        // 6 dB boost at 60 Hz with 1/3-octave bw via JSON facade (constant params).
        let json = r#"{
            "register": {
                "clock": ["Clock", {"value": 20.0, "mode": "Samples"}],
                "pqc": ["ParametricConst", {"gain": 6.0, "bw": 0.333, "freq": 60.0}],
                "r": ["Round", {"places": 3, "mode": "Round"}]
            },
            "connect": [
                ["clock.out", "pqc.in"],
                ["pqc.out", "r.in"]
            ]
        }"#;
        let mut g = GenGraph::new(2000.0, 16);
        let gf: GraphFacade = serde_json::from_str(json).unwrap();
        gf.register_and_connect(&mut g).unwrap();
        g.process();
        assert_eq!(
            g.get_output_by_label("r.out"),
            vec![
                1.015, 0.029, 0.027, 0.024, 0.02, 0.015, 0.01, 0.005, -0.0, -0.005,
                -0.01, -0.014, -0.018, -0.021, -0.023, -0.024,
            ]
        );
    }

    #[test]
    fn test_chain_ugen_reference_markdown() {
        let md = chain_ugen_reference_markdown();

        // Section header is present.
        assert!(md.contains("## Chain DSL UGen Reference"));

        // Every UGFacade variant name appears as a subsection heading.
        for name in [
            "AsHz",
            "BassDrum",
            "Ceil",
            "Clock",
            "Const",
            "EnvAR",
            "EnvBreakPoint",
            "Fade",
            "Floor",
            "HighHat",
            "HighPass",
            "HighPassQ",
            "Lfo",
            "LowPass",
            "LowPassQ",
            "MixLinear",
            "Mult",
            "Pan",
            "Parametric",
            "ParametricConst",
            "PulseSelect",
            "Reverb",
            "Round",
            "Select",
            "Sine",
            "SnareDrum",
            "Sum",
            "Trigger",
            "White",
        ] {
            assert!(
                md.contains(&format!("### {name}")),
                "missing section for {name}"
            );
        }

        // Required and optional args are distinguished.
        assert!(md.contains("*required*"), "expected '*required*' marker");
        assert!(md.contains("**Construction args:**"));
        assert!(md.contains("**Inputs:**"));
        assert!(md.contains("**Outputs:**"));
    }
}
