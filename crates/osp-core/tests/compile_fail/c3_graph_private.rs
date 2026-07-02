// INV-C3 compile-fail: external crate ConceptGraph.nodes field erişemez.
// ConceptGraph.nodes/edges private → harici Accepted write engelli.
use osp_core::anchoring::types::{ConceptGraph, ConceptNode, ConceptNodeId};

fn main() {
    let graph = ConceptGraph::new();
    // Bu satır derlenmemeli: `nodes` field private.
    let _count = graph.nodes.len();
}
