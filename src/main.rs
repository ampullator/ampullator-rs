use ampullator::GenGraph;
use ampullator::SineOscillator;


fn main() {
    let mut graph = GenGraph::new(44100.0, 128);

    graph.add_node("lfo", Box::new(SineOscillator::new()));
    graph.add_node("osc", Box::new(SineOscillator::new()));

    // Connect "lfo" output to "osc" frequency input
    graph.connect_named("osc", "freq", "lfo");


}
