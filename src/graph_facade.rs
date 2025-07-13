use serde::Deserialize;

use crate::GenGraph;
use crate::ModeRound;
use crate::ugen_core::UGen;
use crate::ugen_core::{UGClock, UGSum, UGConst, UGRound, UGWhite};
use crate::ugen_select::{ModeSelect, UGSelect};
use crate::util::Sample;
use crate::util::UnitRate;
use crate::Recorder;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, tag = "0", content = "1")]
pub enum UGFacade {
    Const {
        value: Sample,
    },
    Clock {
        value: Sample,
        mode: UnitRate,
    },
    Select {
        values: Vec<f32>,
        mode: ModeSelect,
        seed: Option<u64>,
    },
    Round {
        places: i32,
        mode: ModeRound,
    },
    Sum {
        input_count: usize,
    },
    White {
        seed: Option<u64>,
    },
}

#[allow(unused)]
impl UGFacade {
    pub fn to_ugen(&self) -> Box<dyn UGen> {
        match self {
            UGFacade::Const { value } => Box::new(UGConst::new(*value)),
            UGFacade::Clock { value, mode } => {
                Box::new(UGClock::new(*value, mode.clone()))
            }
            UGFacade::Select { values, mode, seed } => {
                Box::new(UGSelect::new(values.clone(), *mode, *seed))
            }
            UGFacade::Round { places, mode } => {
                Box::new(UGRound::new(*places, mode.clone()))
            },
            UGFacade::Sum { input_count } => Box::new(UGSum::new(*input_count)),
            UGFacade::White { seed } => Box::new(UGWhite::new(*seed)),

        }
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

pub fn register_many(graph: &mut GenGraph, j: &str) {
    let defs: HashMap<String, Facade> = serde_json::from_str(j).unwrap();
    for (name, def) in defs {
        graph.add_node(name, def.to_ugen());
    }
}

/// Connects nodes in a GenGraph using a JSON string of `"src": "dst"` mappings.
pub fn connect_many(graph: &mut GenGraph, j: &str) {
    let pairs: HashMap<String, String> =
        serde_json::from_str(j).expect("Failed to parse connection JSON");
    for (src, dst) in pairs {
        graph.connect(&src, &dst);
    }
}

//------------------------------------------------------------------------------

#[derive(Deserialize, Debug)]
struct GraphFacade {
    title: Option<String>,
    label: Option<String>,
    register: HashMap<String, Facade>,
    connect: HashMap<String, String>,
}

fn register_and_connect(graph: &mut GenGraph, parsed: &GraphFacade) -> Result<(), String> {
    for (name, facade) in &parsed.register {
        println!("register: {:?}", name);
        graph.add_node(name, facade.to_ugen());
    }
    for (src, dst) in &parsed.connect {
        println!("connect: {:?} -> {:?}", src, dst);
        graph.connect(&src, &dst);
    }
    Ok(())
}

fn from_json_write_figures(
    parsed: &GraphFacade,
    dir: &Path,
    sample_rate: f32,
    buffer_size: usize,
    total_samples: usize,
) -> Result<String, String> {

    println!("from_json_write_figures: dir: {:?}", dir);

    let mut g = GenGraph::new(sample_rate, buffer_size);
    for (name, facade) in &parsed.register {
        println!("register: {:?}: {:?}", name, facade);
        g.add_node(name, facade.to_ugen());
    }
    for (src, dst) in &parsed.connect {
        println!("connect: {:?} -> {:?}", src, dst);
        g.connect(&src, &dst);
    }

    let name = parsed.label.clone().unwrap_or_else(|| "graph".to_string());
    let file_name = format!("{name}_time-domain.png");
    let out_path = dir.join(&file_name);

    println!("from_json_write_figures: {:?}", out_path);

    let r1 = Recorder::from_samples(g, None, total_samples);

    r1.to_gnuplot_fp(out_path.to_str().unwrap()).unwrap();

    Ok(file_name)
}

pub fn build_markdown_index(
        input_dir: &Path,
        output_dir: &Path,
        sample_rate: f32,
        buffer_size: usize,
        total_samples: usize,
    ) -> Result<(), String> {

    let mut entries = Vec::new();
    entries.push("# Ampullator\n\n".to_string());

    std::fs::create_dir_all(output_dir).map_err(|e| e.to_string())?;

    for entry in std::fs::read_dir(input_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {

            println!("build_markdown_index: parsing: {:?}", path);

            let json_str = std::fs::read_to_string(&path).map_err(|e| e.to_string())?.trim().to_string();
            let parsed: GraphFacade = serde_json::from_str(&json_str).map_err(|e| e.to_string())?;

            let title = parsed.title.clone().unwrap_or("title".to_string());
            let label = parsed.label.clone().unwrap_or("label".to_string());

            let fp_name = from_json_write_figures(&parsed, &output_dir, sample_rate, buffer_size, total_samples)?;

            entries.push(format!(
                "## {title}\n```json\n{json_str}\n```\n![{label}]({fp_name})\n",
            ));
        }
    }

    std::fs::write(output_dir.join("index.md"), entries.join("\n")).map_err(|e| e.to_string())?;
    Ok(())
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
        let json = r#"
        {
            "c1": ["Const", {"value": 1.0 }],
            "c2": 4,
            "clock": ["Clock", { "value": 2.0, "mode": "Samples" }],
            "rounder": ["Round", { "places": 2, "mode": "Round" }]
        }
        "#;

        let mut g = GenGraph::new(8.0, 8);
        register_many(&mut g, json);
        assert_eq!(g.len(), 4);
    }

    #[test]
    fn test_ug_facade_c() {
        let jr = r#"
        {
            "step": 1,
            "clock": ["Clock", { "value": 2.0, "mode": "Samples" }],
            "sel": ["Select", { "values": [10, 5, 15, 20], "mode": "Shuffle", "seed": 42 }]
        }
        "#;

        let mut g = GenGraph::new(8.0, 8);
        register_many(&mut g, jr);

        let jc = r#"
        {
          "clock.out": "sel.trigger",
          "step.out": "sel.step"
        }
        "#;
        connect_many(&mut g, jc);
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
            "connect": {
              "clock.out": "sel.trigger",
              "step.out": "sel.step"
            }
        }
        "#;

        let mut g = GenGraph::new(8.0, 8);

        let parsed: GraphFacade =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse JSON: {e}")).unwrap();
        let res = register_and_connect(&mut g, &parsed);
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
    fn test_build_index_a() {
        let fp_src = Path::new("doc/example");
        let fp_dst = Path::new("doc/out");
        let _ = build_markdown_index(&fp_src, &fp_dst, 100.0, 10, 100).unwrap();
    }
}
