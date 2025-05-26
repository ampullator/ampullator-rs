use ampullator::GenGraph;
use ampullator::SineOscillator;



fn main() {
    let mut graph = GenGraph::new(44100.0, 128);

    graph.add_node("lfo", Box::new(SineOscillator::new()));
    graph.add_node("osc", Box::new(SineOscillator::new()));

    // Connect "lfo" 'wave' output to "osc" 'freq' input
    graph.connect_named("osc", "freq", "lfo", "wave");

    // Collect output over multiple frames
    let frames = 10;
    let mut all_wave = Vec::new();
    let mut all_trigger = Vec::new();

    for _ in 0..frames {
        graph.process();
        all_wave.extend_from_slice(graph.get_output("osc", "wave"));
        all_trigger.extend_from_slice(graph.get_output("osc", "trigger"));
    }

    // Print first few results
    println!("First 16 wave samples:");
    for (i, s) in all_wave.iter().take(16).enumerate() {
        println!("wave[{:02}]: {:.5}", i, s);
    }

    println!("\nFirst 16 trigger samples:");
    for (i, s) in all_trigger.iter().take(16).enumerate() {
        println!("trig[{:02}]: {:.5}", i, s);
    }
}
