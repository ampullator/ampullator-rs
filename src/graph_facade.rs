use serde::Deserialize;

use crate::GenGraph;
use crate::ModeRound;
use crate::ugen_core::UGen;
use crate::ugen_core::{UGClock, UGConst, UGRound};
use crate::ugen_select::{ModeSelect, UGSelect};
use crate::util::Sample;
use crate::util::UnitRate;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Facade {
    Short(f32),     // concise numeric constant: "step": 1
    Full(UGFacade), // ["Clock", { ... }] or ["Round", { ... }]
}


pub fn register_many(graph: &mut GenGraph, j: &str) {
    let defs: HashMap<String, Facade> = serde_json::from_str(j).unwrap();
    for (name, def) in defs {
        let ugen: Box<dyn UGen> = match def {
            Facade::Short(f) => Box::new(UGConst::new(f)),
            Facade::Full(facade) => facade.to_ugen(),
        };
        graph.add_node(name, ugen);
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
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
}
