use ampullator::GenGraph;
use ampullator::UGAsHz;
use ampullator::UGConst;
use ampullator::UGSine;
use ampullator::UGSum;
use ampullator::UnitRate;

fn test1() {
    let mut graph = GenGraph::new(44100.0, 128);

    // Add nodes
    graph.add_node("lfo", Box::new(UGSine::new()));
    graph.add_node("osc", Box::new(UGSine::new()));
    graph.add_node("mix", Box::new(UGSum::new(2))); // 2-input sum

    // Connections
    graph.connect_named("lfo.wave", "osc.freq"); // modulate osc with lfo
    graph.connect_named("osc.wave", "mix.in0");
    graph.connect_named("lfo.wave", "mix.in1");

    // Collect output over multiple frames
    let frames = 10;
    let mut all_mix = Vec::new();
    let mut all_trigger = Vec::new();

    for _ in 0..frames {
        graph.process();
        all_mix.extend_from_slice(graph.get_output_named("mix.sum"));
        all_trigger.extend_from_slice(graph.get_output_named("osc.trigger"));
    }

    // Print results
    // println!("First 16 mixed samples:");
    // for (i, s) in all_mix.iter().take(16).enumerate() {
    //     println!("mix[{:02}]: {:.5}", i, s);
    // }

    // println!("\nFirst 16 trigger samples from osc:");
    // for (i, s) in all_trigger.iter().take(16).enumerate() {
    //     println!("trig[{:02}]: {:.5}", i, s);
    // }

    println!("{}", graph.describe());
}

fn test2() {
    let mut graph = GenGraph::new(44100.0, 128);

    // Mock source for MIDI note or BPM (e.g. 60 bpm = 1Hz)
    graph.add_node("note", Box::new(UGConst::new(69.0))); // A4
    graph.add_node("conv", Box::new(UGAsHz::new(UnitRate::Midi)));
    graph.add_node("osc", Box::new(UGSine::new()));

    graph.connect_named("note.out", "conv.in");
    graph.connect_named("conv.hz", "osc.freq");

    for _ in 0..10 {
        graph.process();
        // let wave = graph.get_output("osc", "wave");
        // println!("{:.3?}", &wave[..8.min(wave.len())]);
    }
    println!("{}", graph.describe());
}

fn main() {
    test1();
    test2();
}
