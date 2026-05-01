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
Clock(value=120, mode=Bpm)
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
Clock(value=300, mode=Bpm) => metro
| metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd
| metro -> PulseSelect(duration_values=[1, 2, 1], duration_mode=Shuffle) -> SnareDrum() => sn
| (bd + sn) => mix
```


## CLIs


### Build Readme:

```bash
cargo run --bin ampullator-doc
```

### Record WAV from a chain or graph file:

```bash
cargo run --bin ampullator-record -- "Clock(value=5, mode=Samples)" -o /tmp/out.wav --duration 2
```

On Linux with `aplay` it is possible to omit the output path and pipe WAV:

```bash
cargo run --bin ampullator-record -- "Sine() => s * .4 | 220 ->:freq s" --duration 4 | aplay

cargo run --bin ampullator-record -- "Clock(value=300, mode=Bpm) => metro | metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd | metro -> PulseSelect(duration_values=[1,2,1], duration_mode=Shuffle)-> SnareDrum() => sn | bd + sn" --duration 8 | aplay
```

On MacOS with `sox` `play`:

```bash
cargo run --bin ampullator-record -- "Sine() => s -> Pan() => p &> Fade(channels=2, level=0.7) | Lfo(rate=0.5, wave=Triangle, mode=Seconds, min=220, max=440) ->:freq s | Lfo(rate=2, wave=Triangle, mode=Seconds) ->:pan p" --duration 4 | play -

cargo run --bin ampullator-record -- "Clock(value=300, mode=Bpm) => metro | metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd | metro -> PulseSelect(duration_values=[1,2,1], duration_mode=Shuffle)-> SnareDrum() => sn | bd + sn" --duration 8 | play -
```

## Examples

### Clock Control
```text
(Clock(value=12, mode=Samples) * .5) + (Clock(value=3, mode=Samples) * .33)
```
![ug_clock-control](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-control_graph.svg)
![ug_clock-control](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-control_time-domain.svg)

### Clock Mixture
```text
Clock(value=12, mode=Samples) + Clock(value=5, mode=Samples)
```
![ug_clock-mix](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-mix_graph.svg)
![ug_clock-mix](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_clock-mix_time-domain.svg)

### Drum Trigger
```text
Clock(value=500, mode=Samples) => trigger -> SnareDrum(seed=42) => sd
| trigger -> Select(values=[100, 1000], mode=Cycle) ->:tone_decay sd
```
![ug_drum-trigger](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_drum-trigger_graph.svg)
![ug_drum-trigger](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_drum-trigger_time-domain.svg)

### Pulse Select
```text
Clock(value=1, mode=Samples)=> metro
| Clock(value=20, mode=Samples) -> Select(values=[1, 2, 4], mode=Cycle) => step
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
| Lfo(wave=Triangle, min=22, max=44) ->:rate s
| o
```
![ug_sine-lfo](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_sine-lfo_graph.svg)
![ug_sine-lfo](https://raw.githubusercontent.com/ampullator/ampullator-rs/refs/heads/main/doc/out/ug_sine-lfo_time-domain.svg)

### White Noise Masking
```text
White(seed=42) => noise
| Clock(value=20, mode=Samples) => clock
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
