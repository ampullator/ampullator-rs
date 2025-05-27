use ampullator::GenGraph;
use ampullator::OscSine;
use ampullator::SumNode;
use ampullator::FreqConverterNode;
use ampullator::FreqUnit;
use ampullator::ConstantNode;




fn test1() {
    let mut graph = GenGraph::new(44100.0, 128);

    // Add nodes
    graph.add_node("lfo", Box::new(OscSine::new()));
    graph.add_node("osc", Box::new(OscSine::new()));
    graph.add_node("mix", Box::new(SumNode::new(2))); // 2-input sum

    // Connections
    graph.connect_named("osc", "freq", "lfo", "wave"); // modulate osc with lfo
    graph.connect_named("mix", "in0", "osc", "wave");
    graph.connect_named("mix", "in1", "lfo", "wave");

    // Collect output over multiple frames
    let frames = 10;
    let mut all_mix = Vec::new();
    let mut all_trigger = Vec::new();

    for _ in 0..frames {
        graph.process();
        all_mix.extend_from_slice(graph.get_output("mix", "sum"));
        all_trigger.extend_from_slice(graph.get_output("osc", "trigger"));
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
    graph.add_node("note", Box::new(ConstantNode::new(69.0))); // A4
    graph.add_node("conv", Box::new(FreqConverterNode::new(FreqUnit::Midi)));
    graph.add_node("osc", Box::new(OscSine::new()));

    graph.connect_named("conv", "in", "note", "out");
    graph.connect_named("osc", "freq", "conv", "hz");

    for _ in 0..10 {
        graph.process();
        let wave = graph.get_output("osc", "wave");
        // println!("{:.3?}", &wave[..8.min(wave.len())]);
    }
    println!("{}", graph.describe());
}


fn main() {
    test1();
    test2();
}

