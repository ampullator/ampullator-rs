use std::str::FromStr;

//------------------------------------------------------------------------------
pub type Sample = f32;

pub(crate) fn split_name(s: &str) -> (&str, &str) {
    s.rsplit_once('.')
        .unwrap_or_else(|| panic!("Expected 'name.port', got: '{}'", s))
}

//------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum UnitRate {
    Hz,
    Seconds,
    Samples,
    Midi,
    Bpm,
}

impl FromStr for UnitRate {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hz" => Ok(UnitRate::Hz),
            "sec" | "seconds" => Ok(UnitRate::Seconds),
            "samples" | "spc" => Ok(UnitRate::Samples),
            "midi" => Ok(UnitRate::Midi),
            "bpm" => Ok(UnitRate::Bpm),
            _ => Err(format!("Unknown frequency unit: {}", s)),
        }
    }
}

pub(crate) fn unitrate_to_hz(
    value: Sample,
    mode: UnitRate,
    sample_rate: Sample,
) -> Sample {
    match mode {
        UnitRate::Hz => value,
        UnitRate::Seconds => {
            if value != 0.0 {
                1.0 / value
            } else {
                0.0
            }
        }
        UnitRate::Samples => {
            if value != 0.0 {
                sample_rate / value
            } else {
                0.0
            }
        }
        UnitRate::Midi => 440.0 * 2f32.powf((value - 69.0) / 12.0),
        UnitRate::Bpm => value / 60.0,
    }
}
