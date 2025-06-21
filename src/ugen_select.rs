use crate::UGen;
use crate::util::Sample;

use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng, rngs::StdRng};

//------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub enum ModeSelect {
    Cycle,
    Random,
    Shuffle,
    Walk,
}

#[derive(Clone)]
pub struct UGSelect {
    values: Vec<Sample>,
    mode: ModeSelect,
    index: usize,
    shuffle_remaining: Vec<usize>,
    rng: StdRng,
}

impl UGSelect {
    pub fn new(values: Vec<Sample>, mode: ModeSelect, seed: Option<u64>) -> Self {
        let rng = match seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_rng(&mut rand::rng()),
        };
        let len = values.len().max(1);
        UGSelect {
            values,
            mode,
            index: len - 1, // not optimal
            shuffle_remaining: Vec::new(),
            rng,
        }
    }

    /// Alternate interface to select a single value.
    pub fn select_next(
        &mut self,
        step: Sample,
        sample_rate: f32,
        time_sample: usize,
    ) -> Sample {
        let trigger = [1.0];
        let step = [step];
        let mut out = [0.0];
        let inputs = [&trigger[..], &step[..]];
        let mut outputs = [&mut out[..]];

        self.process(&inputs, &mut outputs, sample_rate, time_sample);
        out[0]
    }
}

impl UGen for UGSelect {
    fn type_name(&self) -> &'static str {
        "UGSelect"
    }

    fn input_names(&self) -> &[&'static str] {
        &["trigger", "step"]
    }

    fn output_names(&self) -> &[&'static str] {
        &["out"]
    }

    fn default_input(&self, input_name: &str) -> Option<Sample> {
        match input_name {
            "trigger" => Some(0.0),
            "step" => Some(1.0),
            _ => None,
        }
    }

    fn describe_config(&self) -> Option<String> {
        Some(format!("values = {:?}, mode = {:?}", self.values, self.mode).to_lowercase())
    }

    fn process(
        &mut self,
        inputs: &[&[Sample]],
        outputs: &mut [&mut [Sample]],
        _sample_rate: f32,
        _time_sample: usize,
    ) {
        let trigger = inputs.get(0).copied().unwrap_or(&[]);
        let step = inputs.get(1).copied().unwrap_or(&[]);
        let out = &mut outputs[0];

        let n = self.values.len();
        if n == 0 {
            for o in out.iter_mut() {
                *o = 0.0;
            }
            return;
        }

        for i in 0..out.len() {
            if trigger.get(i).copied().unwrap_or(0.0) > 0.5 {
                let step_size =
                    step.get(i).copied().unwrap_or(1.0).round().max(1.0) as usize;

                match self.mode {
                    ModeSelect::Cycle => {
                        self.index = (self.index + step_size) % n;
                    }
                    ModeSelect::Random => {
                        self.index = self.rng.random_range(0..n);
                    }
                    ModeSelect::Shuffle => {
                        for _ in 0..step_size {
                            if self.shuffle_remaining.is_empty() {
                                self.shuffle_remaining = (0..n).collect();
                                self.shuffle_remaining.shuffle(&mut self.rng);
                            }
                            self.index = self.shuffle_remaining.pop().unwrap();
                        }
                    }
                    ModeSelect::Walk => {
                        let direction = if self.rng.random_bool(0.5) { 1 } else { -1 };
                        let step_signed = step_size as isize * direction as isize;
                        let new_index = ((self.index as isize + step_signed)
                            .rem_euclid(n as isize))
                            as usize;
                        self.index = new_index;
                    }
                }
            }
            out[i] = self.values[self.index];
        }
    }
}

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Recorder;
    use crate::UnitRate;
    use crate::connect_many;
    use crate::register_many;
    use crate::{GenGraph, UGClock, UGSum};

    //--------------------------------------------------------------------------
    #[test]
    fn test_select_cycle_a() {
        let mut g = GenGraph::new(8.0, 8);
        register_many![g,
            "s1" => UGSelect::new(
                vec![3.0, 10.0, 20.0, 50.0, 999.0],
                ModeSelect::Cycle,
                Some(42)),
            "c1" => 1,
        ];
        g.connect("c1.out", "s1.trigger");
        g.process();

        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![3.0, 10.0, 20.0, 50.0, 999.0, 3.0, 10.0, 20.0]
        )
    }

    #[test]
    fn test_select_cycle_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock1" => UGClock::new(5.0, UnitRate::Samples),
            "clock2" => UGClock::new(13.0, UnitRate::Samples),
            "sum" => UGSum::new(2),
            "step" => 1,
            "sel" => UGSelect::new(
                vec![3.0, 6.0, 12.0, 24.0, 48.0],
                ModeSelect::Cycle,
                Some(42)),
        ];

        connect_many![g,
            "clock1.out" -> "sum.in1",
            "clock2.out" -> "sum.in2",
            "sum.out" -> "sel.trigger",
            "step.out" -> "sel.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![
                3.0, 3.0, 3.0, 3.0, 3.0, 6.0, 6.0, 6.0, 6.0, 6.0, 12.0, 12.0, 12.0, 24.0,
                24.0, 48.0, 48.0, 48.0, 48.0, 48.0, 3.0, 3.0, 3.0, 3.0, 3.0, 6.0, 12.0,
                12.0, 12.0, 12.0, 24.0, 24.0, 24.0, 24.0, 24.0, 48.0, 48.0, 48.0, 48.0,
                3.0, 6.0, 6.0, 6.0, 6.0, 6.0, 12.0, 12.0, 12.0, 12.0, 12.0, 24.0, 24.0,
                48.0, 48.0, 48.0, 3.0, 3.0, 3.0, 3.0, 3.0, 6.0, 6.0, 6.0, 6.0, 6.0, 12.0,
                12.0, 12.0, 12.0, 12.0, 24.0, 24.0, 24.0, 24.0, 24.0, 48.0, 48.0, 48.0,
                3.0, 3.0, 6.0, 6.0, 6.0, 6.0, 6.0, 12.0, 12.0, 12.0, 12.0, 12.0, 24.0,
                48.0, 48.0, 48.0, 48.0, 3.0, 3.0, 3.0, 3.0, 3.0
            ]
        );
    }

    #[test]
    fn test_select_cycle_c() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock1" => UGClock::new(2.0, UnitRate::Samples),
            "clock2" => UGClock::new(9.0, UnitRate::Samples),
            "step" => 1,
            "sel-step" => UGSelect::new(
                vec![1., 2., 3.],
                ModeSelect::Cycle,
                Some(42)),
            "sel-value" => UGSelect::new(
                vec![2., 4., 8., 16., 32.],
                ModeSelect::Cycle,
                Some(42)),
        ];

        connect_many![g,
            "clock1.out" -> "sel-value.trigger",
            "clock2.out" -> "sel-step.trigger",
            "step.out" -> "sel-step.step",
            "sel-step.out" -> "sel-value.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("sel-value.out"),
            vec![
                2.0, 2.0, 4.0, 4.0, 8.0, 8.0, 16.0, 16.0, 32.0, 32.0, 4.0, 4.0, 16.0,
                16.0, 2.0, 2.0, 8.0, 8.0, 2.0, 2.0, 16.0, 16.0, 4.0, 4.0, 32.0, 32.0,
                8.0, 8.0, 16.0, 16.0, 32.0, 32.0, 2.0, 2.0, 4.0, 4.0, 16.0, 16.0, 2.0,
                2.0, 8.0, 8.0, 32.0, 32.0, 4.0, 4.0, 32.0, 32.0, 8.0, 8.0, 2.0, 2.0,
                16.0, 16.0, 32.0, 32.0, 2.0, 2.0, 4.0, 4.0, 8.0, 8.0, 16.0, 16.0, 2.0,
                2.0, 8.0, 8.0, 32.0, 32.0, 4.0, 4.0, 32.0, 32.0, 8.0, 8.0, 2.0, 2.0,
                16.0, 16.0, 4.0, 4.0, 8.0, 8.0, 16.0, 16.0, 32.0, 32.0, 2.0, 2.0, 8.0,
                8.0, 32.0, 32.0, 4.0, 4.0, 16.0, 16.0, 2.0, 2.0
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_select_walk_a() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(2.0, UnitRate::Samples),
            "step" => 1,
            "sel" => UGSelect::new(
                vec![0., 1., 2., 3., 4., 5., 6., 7., 8., 9.],
                ModeSelect::Walk,
                Some(42)),
        ];

        connect_many![g,
            "clock.out" -> "sel.trigger",
            "step.out" -> "sel.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![
                8.0, 8.0, 7.0, 7.0, 6.0, 6.0, 7.0, 7.0, 8.0, 8.0, 9.0, 9.0, 8.0, 8.0,
                7.0, 7.0, 8.0, 8.0, 9.0, 9.0, 8.0, 8.0, 7.0, 7.0, 8.0, 8.0, 9.0, 9.0,
                8.0, 8.0, 9.0, 9.0, 0.0, 0.0, 9.0, 9.0, 0.0, 0.0, 9.0, 9.0, 8.0, 8.0,
                9.0, 9.0, 8.0, 8.0, 9.0, 9.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0,
                2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 1.0, 1.0, 0.0, 0.0, 9.0, 9.0, 8.0, 8.0,
                9.0, 9.0, 8.0, 8.0, 9.0, 9.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 9.0, 9.0,
                0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 3.0, 3.0, 2.0, 2.0,
                1.0, 1.0
            ]
        );
    }

    #[test]
    fn test_select_walk_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock1" => UGClock::new(2.0, UnitRate::Samples),
            "clock2" => UGClock::new(12.0, UnitRate::Samples),
            "step" => 1,
            "sel-step" => UGSelect::new(
                vec![1., 2., 5.],
                ModeSelect::Walk,
                Some(42)),
            "sel-value" => UGSelect::new(
                vec![0., 1., 2., 3., 4., 5., 6., 7., 8., 9.],
                ModeSelect::Walk,
                Some(42)),
        ];

        connect_many![g,
            "clock1.out" -> "sel-value.trigger",
            "clock2.out" -> "sel-step.trigger",
            "step.out" -> "sel-step.step",
            "sel-step.out" -> "sel-value.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("sel-value.out"),
            vec![
                7.0, 7.0, 5.0, 5.0, 3.0, 3.0, 5.0, 5.0, 7.0, 7.0, 9.0, 9.0, 7.0, 7.0,
                6.0, 6.0, 7.0, 7.0, 8.0, 8.0, 7.0, 7.0, 6.0, 6.0, 7.0, 7.0, 2.0, 2.0,
                7.0, 7.0, 2.0, 2.0, 7.0, 7.0, 2.0, 2.0, 7.0, 7.0, 2.0, 2.0, 1.0, 1.0,
                2.0, 2.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 4.0, 4.0, 2.0, 2.0, 4.0, 4.0,
                6.0, 6.0, 4.0, 4.0, 6.0, 6.0, 4.0, 4.0, 2.0, 2.0, 7.0, 7.0, 2.0, 2.0,
                7.0, 7.0, 2.0, 2.0, 7.0, 7.0, 2.0, 2.0, 4.0, 4.0, 2.0, 2.0, 0.0, 0.0,
                2.0, 2.0, 4.0, 4.0, 6.0, 6.0, 8.0, 8.0, 9.0, 9.0, 8.0, 8.0, 7.0, 7.0,
                6.0, 6.0
            ]
        );
    }

    #[test]
    fn test_select_walk_c() {
        let mut g = GenGraph::new(8.0, 16);
        register_many![g,
            "s1" => UGSelect::new(vec![3.0, 10.0, 20.0, 50.0], ModeSelect::Walk, Some(42)),
            "c1" => 1.0,
        ];
        g.connect("c1.out", "s1.trigger");
        g.process();

        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![
                20.0, 10.0, 3.0, 10.0, 20.0, 50.0, 20.0, 10.0, 20.0, 50.0, 20.0, 10.0,
                20.0, 50.0, 20.0, 50.0
            ]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_select_shuffle_a() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![
            g,
            "s1" => UGSelect::new(
                vec![3.0, 10.0, 20.0, 50.0, 99.0],
                ModeSelect::Shuffle,
                Some(42)),
            "c1" => 1,
        ];
        connect_many![g, "c1.out" -> "s1.trigger"];
        g.process();

        assert_eq!(
            g.get_output_by_label("s1.out"),
            vec![
                10.0, 20.0, 50.0, 3.0, 99.0, 20.0, 99.0, 10.0, 50.0, 3.0, 50.0, 20.0,
                10.0, 3.0, 99.0, 10.0, 50.0, 3.0, 99.0, 20.0
            ]
        );
        assert_eq!(
            format!("\n{}", g.describe()),
            r#"
c1 <UGConst {value = 1.000}>
→ out ≊ 1.000

s1 <UGSelect {values = [3.0, 10.0, 20.0, 50.0, 99.0], mode = shuffle}>
trigger ← c1.out
step ←= 1.000
→ out ≊ 20.000
"#
        );
    }

    #[test]
    fn test_select_shuffle_b() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(2.0, UnitRate::Samples),
            "step" => 1,
            "sel" => UGSelect::new(
                vec![0., 1., 2., 3., 4., 5., 6., 7., 8., 9.],
                ModeSelect::Shuffle,
                Some(42)),
        ];

        connect_many![g,
            "clock.out" -> "sel.trigger",
            "step.out" -> "sel.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![
                0.0, 0.0, 4.0, 4.0, 7.0, 7.0, 9.0, 9.0, 5.0, 5.0, 1.0, 1.0, 2.0, 2.0,
                3.0, 3.0, 6.0, 6.0, 8.0, 8.0, 0.0, 0.0, 6.0, 6.0, 2.0, 2.0, 5.0, 5.0,
                9.0, 9.0, 7.0, 7.0, 4.0, 4.0, 1.0, 1.0, 3.0, 3.0, 8.0, 8.0, 2.0, 2.0,
                9.0, 9.0, 6.0, 6.0, 4.0, 4.0, 3.0, 3.0, 5.0, 5.0, 8.0, 8.0, 1.0, 1.0,
                0.0, 0.0, 7.0, 7.0, 1.0, 1.0, 3.0, 3.0, 6.0, 6.0, 9.0, 9.0, 4.0, 4.0,
                7.0, 7.0, 8.0, 8.0, 0.0, 0.0, 5.0, 5.0, 2.0, 2.0, 2.0, 2.0, 3.0, 3.0,
                7.0, 7.0, 9.0, 9.0, 5.0, 5.0, 0.0, 0.0, 1.0, 1.0, 8.0, 8.0, 6.0, 6.0,
                4.0, 4.0
            ]
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_select_random_a() {
        let mut g = GenGraph::new(8.0, 20);
        register_many![g,
            "clock" => UGClock::new(2.0, UnitRate::Samples),
            "step" => 1,
            "sel" => UGSelect::new(
                vec![0., 1., 2., 3., 4., 5., 6., 7., 8., 9.],
                ModeSelect::Random,
                Some(42)),
        ];

        connect_many![g,
            "clock.out" -> "sel.trigger",
            "step.out" -> "sel.step",
        ];

        let r1 = Recorder::from_samples(g, None, 100);
        // r1.to_gnuplot_fp("/tmp/ampullator.png").unwrap();

        assert_eq!(
            r1.get_output_by_label("sel.out"),
            vec![
                1.0, 1.0, 5.0, 5.0, 2.0, 2.0, 5.0, 5.0, 8.0, 8.0, 6.0, 6.0, 9.0, 9.0,
                4.0, 4.0, 9.0, 9.0, 0.0, 0.0, 6.0, 6.0, 4.0, 4.0, 3.0, 3.0, 7.0, 7.0,
                1.0, 1.0, 8.0, 8.0, 6.0, 6.0, 1.0, 1.0, 4.0, 4.0, 0.0, 0.0, 8.0, 8.0,
                9.0, 9.0, 5.0, 5.0, 5.0, 5.0, 8.0, 8.0, 3.0, 3.0, 7.0, 7.0, 1.0, 1.0,
                8.0, 8.0, 5.0, 5.0, 8.0, 8.0, 2.0, 2.0, 6.0, 6.0, 0.0, 0.0, 1.0, 1.0,
                5.0, 5.0, 4.0, 4.0, 0.0, 0.0, 6.0, 6.0, 6.0, 6.0, 2.0, 2.0, 8.0, 8.0,
                4.0, 4.0, 4.0, 4.0, 9.0, 9.0, 5.0, 5.0, 4.0, 4.0, 0.0, 0.0, 0.0, 0.0,
                3.0, 3.0
            ]
        );
    }
}
