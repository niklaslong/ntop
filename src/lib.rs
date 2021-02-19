// Quick think about how this is going to go.
//
// 1. The data collection should be node agnostic. Perhaps a peers trait could be implemented?
//    Methods would be `active_connections` returning some form of generic serde serialisable ID allowing
//    link creation (could be SocketAddr, could be something else).
//
// 2. Aggregation probably needs to be parallelised (rayon?).
//
// 3. Graph is the main returned data structure. Diff could be envisaged for D3. Custom nannou impl
//    might not need it.

use pea2pea::{connect_nodes, Node, NodeConfig, Pea2Pea, Topology};
use std::{collections::HashSet, net::SocketAddr};

const N: usize = 5;

struct JustANode(pub Node);

impl Pea2Pea for JustANode {
    fn node(&self) -> &Node {
        &self.0
    }
}

impl std::ops::Deref for JustANode {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Hash, Eq, PartialEq, Debug)]
struct Vertex {
    id: SocketAddr,
}

#[derive(Hash, Eq, PartialEq, Debug)]
struct Edge {
    source: SocketAddr,
    target: SocketAddr,
}

#[derive(Debug)]
struct Graph {
    vertices: HashSet<Vertex>,
    edges: HashSet<Edge>,
}

impl Graph {
    fn from(nodes: &[JustANode]) -> Self {
        let mut vertices = HashSet::new();
        let mut edges = HashSet::new();

        // Used only for dedup purposes.
        let mut connected_pairs = HashSet::new();

        for node in nodes {
            let own_addr = node.listening_addr();
            vertices.insert(Vertex { id: own_addr });

            let peers_guard = node.known_peers().read();

            for (addr, _) in peers_guard.iter() {
                if own_addr != *addr
                    && connected_pairs.insert((own_addr, *addr))
                    && connected_pairs.insert((*addr, own_addr))
                {
                    edges.insert(Edge {
                        source: own_addr,
                        target: *addr,
                    });
                }
            }
        }

        Self { vertices, edges }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn it_works() {
        // 1. Spawn a bunch of nodes.
        // 2. Collect and track their state.

        let mut nodes = Vec::with_capacity(N);

        let config = NodeConfig {
            listener_ip: "127.0.0.1".parse().unwrap(),
            ..Default::default()
        };

        for _ in 0..N {
            nodes.push(Node::new(Some(config.clone())).await.unwrap());
        }

        let nodes: Vec<JustANode> = nodes.into_iter().map(JustANode).collect();

        connect_nodes(&nodes, Topology::Ring).await;
        dbg!(Graph::from(&nodes));
    }
}
