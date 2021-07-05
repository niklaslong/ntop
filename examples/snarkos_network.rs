use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Read;
use std::net::SocketAddr;
use std::sync::Arc;

use ntop::AsGraph;
use parking_lot::Mutex;
use parking_lot::RwLock;
use serde::Deserialize;

const RPC_PORT: u16 = 3030;

#[tokio::main]
async fn main() {
    let nodes = Arc::new(RwLock::new(Vec::new()));

    nodes.write().push(Node {
        addr: "50.18.246.201:4131".parse().unwrap(),
        rpc: "50.18.246.201:3030".parse().unwrap(),
        peers: vec![],
        is_miner: false,
        is_syncing: false,
    });
    // ntop::start_rpc_server(nodes.clone()).await;

    // Start crawl task.
    let nodes_clone = nodes.clone();
    tokio::spawn(async move {
        let mut count = 0;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            update_nodes(nodes_clone.clone()).await;

            count += 1;
            println!("Completed loop: {}", count);

            let miner_count =
                nodes
                    .read()
                    .iter()
                    .fold(0, |acc, node| if node.is_miner { acc + 1 } else { acc });

            println!("MINER COUNT: {}", miner_count);
            println!("NODE COUNT: {}", nodes.read().len());
        }
    })
    .await;
}

#[derive(Deserialize, Debug)]
struct NodeInfoResponse {
    jsonrpc: String,
    id: String,
    result: NodeInfo,
}

#[derive(Deserialize, Debug)]
struct NodeInfo {
    is_miner: bool,
    is_syncing: bool,
}

#[derive(Deserialize, Debug)]
struct PeerInfoResponse {
    jsonrpc: String,
    id: String,
    result: PeerInfo,
}

#[derive(Deserialize, Debug)]
struct PeerInfo {
    peers: Vec<SocketAddr>,
}

// We need:
//
// 1. Logic to crawl the network.
// 2. A list of nodes we pass to the graph and update as we crawl.

#[derive(Debug)]
struct Node {
    addr: SocketAddr,
    rpc: SocketAddr,
    peers: Vec<SocketAddr>,
    is_miner: bool,
    is_syncing: bool,
}

impl AsGraph for Node {
    type Id = SocketAddr;

    fn id(&self) -> Self::Id {
        self.addr
    }

    fn is_bootnode(&self) -> bool {
        false
    }

    fn active_connections(&self) -> Vec<Self::Id> {
        self.peers.clone()
    }
}

async fn update_nodes(nodes: Arc<RwLock<Vec<Node>>>) {
    // 1. Make the request for each address, collect the returned data mapped to the address.
    // 2. Update the nodes.

    let addrs: HashSet<SocketAddr> = nodes.read().iter().map(|node| node.rpc).collect();
    let mut handles = vec![];
    for (i, rpc) in addrs.clone().into_iter().enumerate() {
        let addrs_clone = addrs.clone();
        let nodes_clone = nodes.clone();
        // curl --data-binary '{"jsonrpc": "2.0", "id":"documentation", "method": "getnodeinfo", "params": [] }' -H 'content-type: application/json' http://127.0.0.1:3030/
        // curl --data-binary '{"jsonrpc": "2.0", "id":"documentation", "method": "getpeerinfo", "params": [] }' -H 'content-type: application/json' http://127.0.0.1:3030/

        let handle = tokio::task::spawn(async move {
            let mut data_info = r#"{"jsonrpc": "2.0", "id":"documentation", "method": "getnodeinfo", "params": []}"#;
            let mut data_peers = r#"{"jsonrpc": "2.0", "id":"documentation", "method": "getpeerinfo", "params": []}"#;

            let client = reqwest::Client::new();
            let node_info_res = client
                .post(format!("http://{}", rpc.clone()))
                .timeout(std::time::Duration::from_secs(5))
                .body(data_info)
                .header("content-type", "application/json")
                .send()
                .await;

            let node_info_response = match node_info_res {
                Err(err) => return,
                Ok(res) => res.json::<NodeInfoResponse>().await.unwrap(),
            };

            let client = reqwest::Client::new();
            let peer_info_res = client
                .post(format!("http://{}", rpc.clone()))
                .timeout(std::time::Duration::from_secs(5))
                .body(data_peers)
                .header("content-type", "application/json")
                .send()
                .await;

            let peer_info_response = match peer_info_res {
                Err(err) => return,
                Ok(res) => res.json::<PeerInfoResponse>().await.unwrap(),
            };

            let node_info = node_info_response.result;
            let peer_info = peer_info_response.result;

            // Update node info.
            let mut nodes_write = nodes_clone.write();
            nodes_write[i].is_miner = node_info.is_miner;
            nodes_write[i].is_syncing = node_info.is_syncing;
            nodes_write[i].peers = peer_info.peers.clone();

            // Create and push new nodes based on the peer addresses.
            for addr in peer_info.peers {
                if !addrs_clone.contains(&addr) && addr.port() == 4131 {
                    nodes_write.push(Node {
                        addr,
                        rpc: SocketAddr::new(addr.ip(), RPC_PORT),
                        // Defaults, when nodes are queried in next loop.
                        peers: vec![],
                        is_miner: false,
                        is_syncing: false,
                    })
                }
            }

            drop(nodes_write);
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await;
    }
}
