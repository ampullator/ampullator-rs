# ampullator-rs
Dynamic generators of complex signals and shapes


## Chain DSL

The Chain DSL is a compact single-string notation for building signal graphs. A chain is parsed into a set of named nodes and a set of directed connections, which are then used to instantiate a `GenGraph`.

Chains can be passed directly on the command line, or stored in a `.txt` or `.chain` file.


### Segments

A chain is one or more **segments** separated by `|`. Each segment defines nodes and/or connections independently; `|` is purely a separator with no audio meaning.

```
White() => noise | LowPass() => lpf | noise -> lpf
```


### UGen instantiation

A UGen is created by writing its type name, optionally followed by keyword arguments in parentheses. Argument values are numbers, identifiers (for enum variants), or lists.

```
Clock(rate=120, mode=Bpm)
ParametricConst(gain=6, bw=0.333, freq=1000)
PulseSelect(duration_values=[3, 2, 1], duration_mode=Cycle)
```

Arguments with defaults can be omitted entirely:

```
LowPass()           # uses default roll_off_db
White()             # no required args
```


### Naming nodes

Any atom can be assigned a name with `=>`. The name can then be referenced in later segments.

```
White() => noise | LowPass() => lpf | noise -> lpf
```

Without `=>`, auto-generated internal names are used and the node cannot be referenced later.


### Connections

`->` connects the default output of the left node to the default input of the right node.

```
White() => noise -> LowPass() => lpf -> HighPass() => hpf
```

Chains of `->` are read left to right; each arrow adds one connection.


### Port specifications

An optional port spec after `->` selects non-default ports. It takes the form `src:dst`, where either side can be omitted to keep the default.

```
# named destination port only (default source output)
4000 ->:cutoff lpf

# named source port only (default destination input)
osc ->wave: recorder

# both ports explicit
noise ->out:in lpf
```


### Multi-signal connections (`&>`)

`&>` connects **all outputs** of the left node to the **first N inputs** of the right node in contiguous order.  The source must have more than one output (otherwise use `->` instead).

```
Sine() -> Pan() &> Reverb() => rev
# Connects pan.out1 -> rev.in_l and pan.out2 -> rev.in_r
```

An optional **multi-port spec** controls which ports are wired. It is a comma-separated list of `src_out:dst_in` pairs placed between `&>` and the destination node. Either side of `:` may be omitted; an omitted output defaults to the n-th contiguous output and an omitted input defaults to the n-th contiguous input.

```
# fully explicit
pan &>out1:in_l,out2:in_r Reverb()

# omit source outputs (default to first N outputs of pan)
pan &>:in_l,:in_r Reverb()

# omit destination inputs (default to first N inputs of Reverb)
pan &>out1:,out2: Reverb()
```

Multiple `&>` operators can be chained:

```
Sine() -> Pan() &> Reverb() &> Fade(channels=2) => fd
```


### Numeric literals

A bare number creates an implicit constant node. This is shorthand for `Const(value=…)`.

```
440 ->:freq osc     # same as: Const(value=440) => c | c ->:freq osc
```


### Binary operators

`+` and `*` wire two nodes into an implicit `Sum` or `Mult` node respectively. `^` wires two nodes into an implicit `Fade` node, connecting the left operand to `in1` and the right operand to `level`. Parentheses control grouping.

```
(a + b)             # sum of a and b
(a * b)             # product of a and b
(a ^ b)             # fade: a scaled by level b
(a + b) => mix      # name the result
```

A full mixing example:

```
Sine() => a | Sine() => b | 330 ->:freq a | 440 ->:freq b | (a + b) => mix
```


### Whitespace

All whitespace — spaces, tabs, newlines — is ignored. Long chains can be split across lines freely:

```
White() => noise
    -> LowPass() => lpf
    -> HighPass() => hpf
| 4000 ->:cutoff lpf
| 800  ->:cutoff hpf
```


### Complete examples

Filter chain with named cutoff controls:

```
White(seed=42) => noise -> LowPass() => lpf -> HighPass() => hpf | 4000 ->:cutoff lpf | 800 ->:cutoff hpf
```

Drum machine driven by a clock and pulse selectors:

```
Clock(rate=300, mode=Bpm) => metro
| metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd
| metro -> PulseSelect(duration_values=[1, 2, 1], duration_mode=Shuffle) -> SnareDrum() => sn
| (bd + sn) => mix
```

## UGen Reference

The following UGens are available in the Chain DSL. Each entry lists construction arguments (with defaults), signal inputs (with default values), and signal outputs.

### AsHz

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `mode` | `Hz` \| `Seconds` \| `Samples` \| `Midi` \| `Bpm` | `Hz` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |

**Outputs:** `out`

### BassDrum

**Inputs:**

| Input | Default |
|-------|---------|
| `gate` | `0` |
| `tune` | `55` |
| `decay` | `9000` |
| `punch` | `2.8` |
| `sweep_decay` | `1200` |
| `click` | `0.2` |
| `tone` | `1` |
| `drive` | `1.3` |

**Outputs:** `out`

### Ceil

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |

**Outputs:** `out`

### Clock

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `rate` | number | *required* |
| `mode` | `Hz` \| `Seconds` \| `Samples` \| `Midi` \| `Bpm` | *required* |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | `1` |

**Outputs:** `out`

### Const

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `value` | number | *required* |

**Outputs:** `out`

### EnvAR

**Inputs:**

| Input | Default |
|-------|---------|
| `trigger` | `0` |
| `attack_dur` | `1` |
| `release_dur` | `1` |
| `attack_curve` | `1` |
| `release_curve` | `1` |

**Outputs:** `out`

### EnvBreakPoint

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `duration_values` | [number, ...] | *required* |
| `duration_mode` | `Cycle` \| `Random` \| `Shuffle` \| `Walk` | *required* |
| `level_values` | [number, ...] | *required* |
| `level_mode` | `Cycle` \| `Random` \| `Shuffle` \| `Walk` | *required* |
| `seed` | integer | `none` |

**Inputs:**

| Input | Default |
|-------|---------|
| `clock` | `0` |
| `step` | `1` |

**Outputs:** `out`

### Fade

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `channels` | integer | `1` |
| `level` | number | `1.0` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in1` | — |
| `level` | `1` |

**Outputs:** `out1`

### Floor

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |

**Outputs:** `out`

### HighHat

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `seed` | integer | `none` |

**Inputs:**

| Input | Default |
|-------|---------|
| `gate` | `0` |
| `tune` | `3969` |
| `decay` | `4000` |
| `tone` | `8000` |
| `accent` | `0.8` |
| `noise` | `0.2` |
| `drive` | `1.2` |

**Outputs:** `out`

### HighPass

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `roll_off_db` | number | `6.0` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |
| `cutoff` | — |

**Outputs:** `out`

### HighPassQ

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `roll_off_db` | number | `6.0` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |
| `cutoff` | — |
| `resonance` | — |

**Outputs:** `out`

### Lfo

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `wave` | `Sine` \| `Triangle` \| `Square` | *required* |
| `rate` | number | `1.0` |
| `mode` | `Hz` \| `Seconds` \| `Samples` \| `Midi` \| `Bpm` | `Hz` |
| `duty` | number | `0.5` |
| `min` | number | `0.0` |
| `max` | number | `1.0` |

**Inputs:**

| Input | Default |
|-------|---------|
| `rate` | `1` |
| `duty` | `0.5` |
| `min` | `0` |
| `max` | `1` |

**Outputs:** `wave`

### LowPass

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `roll_off_db` | number | `6.0` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |
| `cutoff` | — |

**Outputs:** `out`

### LowPassQ

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `roll_off_db` | number | `6.0` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |
| `cutoff` | — |
| `resonance` | — |

**Outputs:** `out`

### MixLinear

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `inputs` | integer | `2` |
| `outputs` | integer | `2` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in1` | — |
| `pan1` | `0.5` |
| `level1` | `1` |
| `in2` | — |
| `pan2` | `0.5` |
| `level2` | `1` |

**Outputs:** `out1`, `out2`

### Mult

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `inputs` | integer | `2` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in1` | — |
| `in2` | — |

**Outputs:** `out`

### Pan

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `outputs` | integer | `2` |
| `pan` | number | `0.5` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |
| `pan` | `0.5` |

**Outputs:** `out1`, `out2`

### Parametric

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |
| `gain` | — |
| `bw` | — |
| `freq` | — |

**Outputs:** `out`

### ParametricConst

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `gain` | number | *required* |
| `bw` | number | *required* |
| `freq` | number | *required* |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |

**Outputs:** `out`

### PulseSelect

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `duration_values` | [number, ...] | *required* |
| `duration_mode` | `Cycle` \| `Random` \| `Shuffle` \| `Walk` | *required* |
| `seed` | integer | `none` |

**Inputs:**

| Input | Default |
|-------|---------|
| `clock` | `0` |
| `step` | `1` |

**Outputs:** `out`

### Reverb

**Inputs:**

| Input | Default |
|-------|---------|
| `in_l` | `0` |
| `in_r` | `0` |
| `decay` | `0.6` |
| `pre_delay` | `20` |
| `mix` | `0.35` |
| `size` | `1` |
| `diffusion` | `0.75` |
| `damping` | `7000` |

**Outputs:** `out_l`, `out_r`

### Round

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `places` | integer | `0` |
| `mode` | `Round` \| `Floor` \| `Ceil` | `Round` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in` | — |

**Outputs:** `out`

### Select

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `values` | [number, ...] | *required* |
| `mode` | `Cycle` \| `Random` \| `Shuffle` \| `Walk` | *required* |
| `seed` | integer | `none` |

**Inputs:**

| Input | Default |
|-------|---------|
| `trigger` | `0` |
| `step` | `1` |

**Outputs:** `out`

### Sine

**Inputs:**

| Input | Default |
|-------|---------|
| `freq` | `440` |
| `phase` | `0` |
| `min` | `-1` |
| `max` | `1` |

**Outputs:** `wave`, `trigger`

### SnareDrum

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `seed` | integer | `none` |

**Inputs:**

| Input | Default |
|-------|---------|
| `gate` | `0` |
| `tune` | `180` |
| `tone` | `0.7` |
| `snappy` | `0.9` |
| `tone_decay` | `3000` |
| `snappy_decay` | `5000` |
| `noise_filter` | `4000` |
| `pitch_sweep` | `1.5` |

**Outputs:** `out`

### Sum

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `inputs` | integer | `2` |

**Inputs:**

| Input | Default |
|-------|---------|
| `in1` | — |
| `in2` | — |

**Outputs:** `out`

### Trigger

**Inputs:**

| Input | Default |
|-------|---------|
| `freq` | `1` |

**Outputs:** `out`

### White

**Construction args:**

| Arg | Type | Default |
|-----|------|---------|
| `seed` | integer | `none` |

**Inputs:**

| Input | Default |
|-------|---------|
| `min` | `-1` |
| `max` | `1` |

**Outputs:** `out`



## CLIs


### Build Readme:

```bash
cargo run --bin ampullator-doc
```

### Record WAV from a chain or graph file:

```bash
cargo run --bin ampullator-record -- "Clock(rate=5, mode=Samples)" -o /tmp/out.wav --duration 2
```

On Linux with `aplay` it is possible to omit the output path and pipe WAV:

```bash
cargo run --bin ampullator-record -- "Sine() => s * .4 | 220 ->:freq s" --duration 4 | aplay

cargo run --bin ampullator-record -- "Clock(rate=300, mode=Bpm) => metro | metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd | metro -> PulseSelect(duration_values=[1,2,1], duration_mode=Shuffle)-> SnareDrum() => sn | bd + sn" --duration 8 | aplay
```

On MacOS with `sox` `play`:

```bash
cargo run --bin ampullator-record -- "Sine() => s * .4 | 220 ->:freq s" --duration 4 | play -

cargo run --bin ampullator-record -- "Clock(rate=300, mode=Bpm) => metro | metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd | metro -> PulseSelect(duration_values=[1,2,1], duration_mode=Shuffle)-> SnareDrum() => sn | bd + sn" --duration 8 | play -
```

## Examples

### Clock Control
```text
(Clock(rate=12, mode=Samples) * .5) + (Clock(rate=3, mode=Samples) * .33)
```
![ug_clock-control](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-control_graph.svg)
![ug_clock-control](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-control_time-domain.svg)

### Clock Mixture
```text
Clock(rate=12, mode=Samples) + Clock(rate=5, mode=Samples)
```
![ug_clock-mix](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-mix_graph.svg)
![ug_clock-mix](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-mix_time-domain.svg)

### Drum Trigger
```text
Clock(rate=500, mode=Samples) => trigger -> SnareDrum(seed=42) => sd
| trigger -> Select(values=[100, 1000], mode=Cycle) ->:tone_decay sd
```
![ug_drum-trigger](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_drum-trigger_graph.svg)
![ug_drum-trigger](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_drum-trigger_time-domain.svg)

### Linear Mixer of Clocks
```text
MixLinear(inputs=4, outputs=2) => mix &> Fade(channels=2) => mlevel
| Clock(rate=20, mode=Samples) ->:in1 mix
| Clock(rate=33, mode=Samples) ->:in2 mix
| Clock(rate=13, mode=Samples) ->:in3 mix
| Clock(rate=45, mode=Samples) ->:in4 mix
| Lfo(rate=12, mode=Samples, wave=Sine) ->:level1 mix
| Lfo(rate=30, mode=Samples, wave=Square) ->:pan2 mix
| Lfo(rate=25, mode=Samples, wave=Triangle) ->:level3 mix
| Lfo(rate=12, mode=Samples, wave=Triangle) ->:pan4 mix
```
![ug_mix_linear-clocks](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_mix_linear-clocks_graph.svg)
![ug_mix_linear-clocks](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_mix_linear-clocks_time-domain.svg)

### Panning Sine with LFO
```text
Sine() => s -> Pan() => p &> Fade(channels=2, level=0.7)
| Lfo(rate=50, wave=Triangle, mode=Samples, min=3, max=6) ->:freq s
| Lfo(rate=80, wave=Triangle, mode=Samples) ->:pan p
```
![ug_pan-lfo](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_pan-lfo_graph.svg)
![ug_pan-lfo](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_pan-lfo_time-domain.svg)

### Pulse Select
```text
Clock(rate=1, mode=Samples)=> metro
| Clock(rate=20, mode=Samples) -> Select(values=[1, 2, 4], mode=Cycle) => step
| metro -> PulseSelect(duration_values=[2, 4, 8], duration_mode=Cycle) => m1
| step ->:step m1
| metro -> PulseSelect(duration_values=[2, 4, 8, 16], duration_mode=Shuffle, seed=42) => m2
| step ->:step m2
```
![ug_pulse-select](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_pulse-select_graph.svg)
![ug_pulse-select](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_pulse-select_time-domain.svg)

### Sine with LFO Control
```text
((Sine() => s) ^ Lfo(rate=0.666, wave=Triangle)) => o
| Lfo(wave=Triangle, min=22, max=44) ->:freq s
| o
```
![ug_sine-lfo](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_sine-lfo_graph.svg)
![ug_sine-lfo](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_sine-lfo_time-domain.svg)

### White Noise Masking
```text
White(seed=42) => noise
| Clock(rate=20, mode=Samples) => clock
| clock -> Select(values=[5, 50, 25], mode=Cycle) ->:max noise
| clock -> Select(values=[-5, -50, -25], mode=Cycle) ->:min noise
```
![ug_white-mask_select](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_white-mask_select_graph.svg)
![ug_white-mask_select](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_white-mask_select_time-domain.svg)

### White Noise Filtering
```text
White(seed=42) => noise -> LowPass() => lpf -> HighPass() => hpf
| 40 ->:cutoff lpf
| 10 ->:cutoff hpf
```
![ug_white-filtering](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_white-filtering_graph.svg)
![ug_white-filtering](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_white-filtering_time-domain.svg)
