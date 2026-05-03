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

