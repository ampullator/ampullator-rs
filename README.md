# ampullator-rs
Dynamic generators of complex signals and shapes


## CLIs


### Build Readme:

    cargo run --bin ampullator-doc

### Record WAV from a chain or graph file:

    cargo run --bin ampullator-record -- "Clock(value=5, mode=Samples)" /tmp/out.wav --duration 2

