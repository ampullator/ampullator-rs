use serde::Deserialize;

use crate::GenGraph;
use crate::ModeRound;
use crate::ugen_core::UGen;
use crate::ugen_core::{UGClock, UGConst, UGRound};
use crate::ugen_select::{ModeSelect, UGSelect};
use crate::util::Sample;
use crate::util::UnitRate;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(tag = "0", content = "1")]
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
}

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
            }
        }
    }
}

#[derive(Deserialize)]
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

pub fn register_many(graph: &mut GenGraph, j: &str) {
    let defs: HashMap<String, Facade> = serde_json::from_str(j).unwrap();
    for (name, def) in defs {
        graph.add_node(name, def.to_ugen());
    }
}

/// Connects nodes in a GenGraph using a JSON string of `"src": "dst"` mappings.
///
/// Example JSON:
/// ```json
/// {
///   "clock.out": "env.trigger",
///   "step.out": "sel.step"
/// }
/// ```
pub fn connect_many(graph: &mut GenGraph, j: &str) {
    let pairs: HashMap<String, String> =
        serde_json::from_str(j).expect("Failed to parse connection JSON");
    for (src, dst) in pairs {
        graph.connect(&src, &dst);
    }
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
        println!("here: {:?}", defs);
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
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![15.0, 15.0, 20.0, 20.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 5.0, 5.0, 20.0, 20.0, 10.0, 10.0, 15.0, 15.0, 5.0, 5.0, 10.0, 10.0, 20.0, 20.0, 20.0, 20.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 5.0, 5.0, 20.0, 20.0, 15.0, 15.0, 10.0, 10.0, 15.0, 15.0, 5.0, 5.0, 10.0, 10.0, 20.0, 20.0, 10.0, 10.0, 5.0, 5.0, 20.0, 20.0, 15.0, 15.0, 5.0, 5.0, 20.0, 20.0, 15.0, 15.0, 10.0, 10.0, 10.0, 10.0, 15.0, 15.0, 20.0, 20.0, 5.0, 5.0, 15.0, 15.0, 20.0, 20.0, 10.0, 10.0, 5.0, 5.0, 15.0, 15.0, 10.0, 10.0, 20.0, 20.0, 5.0, 5.0, 15.0, 15.0, 20.0, 20.0, 5.0, 5.0, 10.0, 10.0, 20.0, 20.0, 5.0, 5.0]
        );

    }
}
