## SnarkOS viz example

The `AsGraph` trait needs to be implemented on `Node`.

```rust
// network/lib.rs

use ntop::AsGraph;

impl AsGraph for Node {
    type Id = SocketAddr;

    fn id(&self) -> Self::Id {
        self.local_address().unwrap()
    }

    fn is_bootnode(&self) -> bool {
        self.environment.is_bootnode()
    }

    fn active_connections(&self) -> Vec<Self::Id> {
        self.peer_book.read().connected_peers().keys().copied().collect()
    }
}
```

The rpc server backing the graph can then be started. Note, the list of nodes tracked by the graph needs to be wrapped in an `Arc` and a `RwLock`. This allows for mutating the list in tests. When the test is running open `d3/index.html` and refresh until the nodes appear.

```rust
#[tokio::test(flavor = "multi_thread")]
async fn mesh() {
    use parking_lot::RwLock;
    use std::sync::Arc;

    let setup = TestSetup {
        consensus_setup: None,
        peer_sync_interval: 5,
        min_peers: MIN_PEERS,
        max_peers: MAX_PEERS,
        ..Default::default()
    };

    // Set up the graphing.
    let nodes = Arc::new(RwLock::new(test_nodes(N, setup).await));
    ntop::start_rpc_server(nodes.clone()).await;

    connect_nodes(&mut nodes.write(), Topology::Mesh).await;
    start_nodes(&nodes.read()).await;

    // Sleep to have time to view the graph before the test exits.
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // ...
}

```


