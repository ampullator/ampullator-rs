use serde::Deserialize;

use crate::util::Sample;
use crate::util::UnitRate;
use crate::ugen_core::UGen;
use crate::ugen_select::{UGSelect, ModeSelect};
use crate::ugen_core::{UGClock, UGRound, UGConst};
use crate::ModeRound;



#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum UGDef {
    Const(f32),

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

impl UGDef {
    pub fn to_ugen(&self) -> Box<dyn UGen> {
        match self {
            UGDef::Const(value) => {
                Box::new(UGConst::new(*value))
            }
            UGDef::Clock { value, mode } => {
                Box::new(UGClock::new(*value, mode.clone()))
            }
            UGDef::Select { values, mode, seed } => {
                Box::new(UGSelect::new(values.clone(), *mode, *seed))
            }
            UGDef::Round { places, mode } => {
                Box::new(UGRound::new(*places, mode.clone()))
            }
        }
    }
}
