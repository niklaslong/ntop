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

use serde::Serialize;

use std::collections::HashSet;
use std::hash::Hash;

#[derive(Debug, Eq, Hash, PartialEq, Serialize, Copy, Clone)]
struct Vertex<T> {
    id: T,
    is_bootnode: bool,
}

#[derive(Debug, Eq, Hash, PartialEq, Serialize, Copy, Clone)]
struct Edge<T> {
    source: T,
    target: T,
}

#[derive(Debug, Serialize)]
pub struct Graph<T: Hash + Eq> {
    vertices: HashSet<Vertex<T>>,
    edges: HashSet<Edge<T>>,
}

#[derive(Debug, Serialize)]
pub struct GraphDiff<T> {
    added_vertices: Vec<Vertex<T>>,
    removed_vertices: Vec<Vertex<T>>,
    added_edges: Vec<Edge<T>>,
    removed_edges: Vec<Edge<T>>,
}

impl<T: Eq + Hash + Copy> Graph<T> {
    pub fn new() -> Self {
        Self {
            vertices: HashSet::new(),
            edges: HashSet::new(),
        }
    }

    pub fn prune_edges(&mut self) {
        let vertice_ids: HashSet<T> = self.vertices.iter().map(|vertice| vertice.id).collect();

        self.edges.retain(|edge| {
            vertice_ids.contains(&edge.source) && vertice_ids.contains(&edge.target)
        });
    }

    fn diff(self: Graph<T>, previous_state: &mut Graph<T>) -> GraphDiff<T> {
        let current_state = self;

        // Compute the diffs.
        let removed_vertices: Vec<Vertex<T>> = previous_state
            .vertices
            .difference(&current_state.vertices)
            .copied()
            .collect();
        let removed_edges: Vec<Edge<T>> = previous_state
            .edges
            .difference(&current_state.edges)
            .copied()
            .collect();

        let added_vertices: Vec<Vertex<T>> = current_state
            .vertices
            .difference(&previous_state.vertices)
            .copied()
            .collect();
        let added_edges: Vec<Edge<T>> = current_state
            .edges
            .difference(&previous_state.edges)
            .copied()
            .collect();

        *previous_state = current_state;

        GraphDiff {
            added_vertices,
            removed_vertices,
            added_edges,
            removed_edges,
        }
    }
}

pub trait AsGraph {
    type Id;

    fn id(&self) -> Self::Id;

    fn is_bootnode(&self) -> bool;

    fn active_connections(&self) -> Vec<Self::Id>;

    fn graph(nodes: &[Self]) -> Graph<Self::Id>
    where
        Self: Sized,
        <Self as AsGraph>::Id: Eq + Hash + Copy,
    {
        let mut vertices = HashSet::new();
        let mut edges = HashSet::new();

        // Used only for dedup purposes.
        let mut connected_pairs = HashSet::new();

        for node in nodes {
            let own_addr = node.id();
            vertices.insert(Vertex {
                id: own_addr,
                is_bootnode: node.is_bootnode(),
            });

            for addr in node.active_connections() {
                if own_addr != addr
                    && connected_pairs.insert((own_addr, addr))
                    && connected_pairs.insert((addr, own_addr))
                {
                    edges.insert(Edge {
                        source: own_addr,
                        target: addr,
                    });
                }
            }
        }

        Graph { vertices, edges }
    }
}

use parking_lot::RwLock;
use std::sync::Arc;
use tokio::task::JoinHandle;

pub async fn start_rpc_server<T: 'static + AsGraph>(nodes: Arc<RwLock<Vec<T>>>) -> JoinHandle<()>
where
    <T as AsGraph>::Id: Eq + Hash + Copy + Serialize + Send + Sync,
    T: Send + Sync,
{
    use ::tokio::task;
    use jsonrpc_core::*;
    use jsonrpc_http_server::*;
    use serde_json::{json, Value};

    let g = Arc::new(RwLock::new(Graph::new()));

    // Listener responds with the current graph every time an RPC call occures.
    let mut io = IoHandler::new();
    io.add_sync_method("graph", move |_| {
        let mut current_state = T::graph(&nodes.read());
        current_state.prune_edges();
        let diff = current_state.diff(&mut g.write());
        Ok(json!(diff))
    });

    let server = ServerBuilder::new(io)
        .cors(DomainsValidation::AllowOnly(vec![
            AccessControlAllowOrigin::Null,
        ]))
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .expect("Unable to start RPC server");

    task::spawn(async {
        server.wait();
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use pea2pea::{connect_nodes, NodeConfig, Topology};
    use pea2pea::{Node, Pea2Pea};

    use std::net::SocketAddr;

    const N: usize = 3;

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

    impl AsGraph for JustANode {
        type Id = SocketAddr;

        fn id(&self) -> Self::Id {
            self.listening_addr()
        }

        fn is_bootnode(&self) -> bool {
            false
        }

        fn active_connections(&self) -> Vec<Self::Id> {
            self.connected_addrs()
        }
    }

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

        let mut nodes: Vec<JustANode> = nodes.into_iter().map(JustANode).collect();

        connect_nodes(&nodes, Topology::Ring).await.unwrap();
        let mut g = JustANode::graph(&nodes);
        g.prune_edges();

        assert_eq!(g.edges.len(), N);

        // Remove the node from the list.
        let node_to_drop = nodes.pop().unwrap();
        let dropped_addr = node_to_drop.listening_addr();
        node_to_drop.shut_down();

        // Disconnect peers from the dropped node.
        for node in &nodes {
            node.disconnect(dropped_addr);
        }

        let mut g = JustANode::graph(&nodes);
        g.prune_edges();

        // Breaking the ring removes 2 connections.
        assert_eq!(g.edges.len(), N - 2);
    }
}
