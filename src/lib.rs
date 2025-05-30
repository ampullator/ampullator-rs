mod graph;
mod ugen_core;
mod util;

pub use ugen_core::{
    ModeRound, UGAsHz, UGConst, UGRound, UGSine, UGSum, UGWhite, UGen, UnitRate,
};

pub use util::Sample;

pub use graph::GenGraph;

pub use graph::plot_graph_to_image;

//------------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gen_graph_describe_json_a() {
        let mut graph = GenGraph::new(44100.0, 128);

        graph.add_node("note", Box::new(UGConst::new(69.0))); // A4
        graph.add_node("conv", Box::new(UGAsHz::new(UnitRate::Midi)));
        graph.add_node("osc", Box::new(UGSine::new()));

        graph.connect("note.out", "conv.in");
        graph.connect("conv.hz", "osc.freq");

        assert_eq!(
            graph.describe_json().to_string(),
            r#"[{"config":"value = 69.000","id":0,"inputs":[],"name":"note","outputs":[{"name":"out","value":0.0}],"type":"UGConst"},{"config":"mode = midi","id":1,"inputs":[{"connected_to":{"node":"note","output":"out"},"name":"in"}],"name":"conv","outputs":[{"name":"hz","value":0.0}],"type":"UGAsHz"},{"config":null,"id":2,"inputs":[{"connected_to":{"node":"conv","output":"hz"},"name":"freq"},{"default":0.0,"name":"phase"},{"default":-1.0,"name":"min"},{"default":1.0,"name":"max"}],"name":"osc","outputs":[{"name":"wave","value":0.0},{"name":"trigger","value":0.0}],"type":"UGSine"}]"#
        );
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_constant_a() {
        let c1 = UGConst::new(3.0);
        assert_eq!(c1.type_name(), "UGConst");

        let mut g = GenGraph::new(120.0, 8);
        g.add_node("c1", Box::new(c1));
        g.process();
        assert_eq!(
            g.get_output_named("c1.out"),
            vec![3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_sum_a() {
        let c1 = UGConst::new(3.0);
        let c2 = UGConst::new(2.0);
        let s1 = UGSum::new(2); // input count

        let mut g = GenGraph::new(120.0, 8);
        g.add_node("c1", Box::new(c1));
        g.add_node("c2", Box::new(c2));
        g.add_node("s1", Box::new(s1));
        g.connect("c1.out", "s1.in1");
        g.connect("c2.out", "s1.in2");
        g.process();

        assert_eq!(
            g.get_output_named("s1.out"),
            vec![5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0, 5.0]
        )
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_sine_a() {
        let c1 = UGConst::new(1.0);
        let osc1 = UGSine::new();
        let r1 = UGRound::new(1, ModeRound::Round);

        let mut g = GenGraph::new(8.0, 8);
        g.add_node("c1", Box::new(c1));
        g.add_node("osc1", Box::new(osc1));
        g.add_node("r1", Box::new(r1));

        g.connect("c1.out", "osc1.freq");
        g.connect("osc1.wave", "r1.in");

        g.process();

        assert_eq!(
            g.get_output_named("r1.out"),
            vec![0.7, 1.0, 0.7, -0.0, -0.7, -1.0, -0.7, 0.0]
        );

        plot_graph_to_image(&g, "/tmp/ampullator.png").unwrap();
    }

    //--------------------------------------------------------------------------
    #[test]
    fn test_white_a() {
        let n1 = UGWhite::new(Some(42));
        let r1 = UGRound::new(2, ModeRound::Round);

        let mut g = GenGraph::new(8.0, 8);
        g.add_node("n1", Box::new(n1));
        g.add_node("r1", Box::new(r1));
        g.connect("n1.out", "r1.in");

        g.process();

        assert_eq!(
            g.get_output_named("r1.out"),
            vec![-0.73, 0.05, -0.5, 0.09, 0.74, 0.27, 0.98, -0.19]
        )
    }
}
