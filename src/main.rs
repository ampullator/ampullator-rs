use ampullator::GenGraph;
use ampullator::SineOscillator;
use ampullator::SumNode;

fn main() {
    let mut graph = GenGraph::new(44100.0, 128);

    // Add nodes
    graph.add_node("lfo", Box::new(SineOscillator::new()));
    graph.add_node("osc", Box::new(SineOscillator::new()));
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
    println!("First 16 mixed samples:");
    for (i, s) in all_mix.iter().take(16).enumerate() {
        println!("mix[{:02}]: {:.5}", i, s);
    }

    println!("\nFirst 16 trigger samples from osc:");
    for (i, s) in all_trigger.iter().take(16).enumerate() {
        println!("trig[{:02}]: {:.5}", i, s);
    }
}
