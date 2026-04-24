# ampullator-rs
Dynamic generators of complex signals and shapes


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
cargo run --bin ampullator-record -- "Sine() => s * .4 | 220 ->:freq s" --duration 4 | play -

cargo run --bin ampullator-record -- "Clock(value=300, mode=Bpm) => metro | metro -> PulseSelect(duration_values=[3, 2, 3], duration_mode=Cycle) -> BassDrum() => bd | metro -> PulseSelect(duration_values=[1,2,1], duration_mode=Shuffle)-> SnareDrum() => sn | bd + sn" --duration 8 | play -
```