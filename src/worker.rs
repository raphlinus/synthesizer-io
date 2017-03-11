// Copyright 2017 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A worker, designed to produce audio in a lock-free manner.

use queue::{Queue, Sender, Receiver, Item};
use module::Buffer;
use graph::{Graph, Node};

pub struct Worker {
    to_worker: Receiver<Node>,
    from_worker: Sender<Node>,
    graph: Graph,
    root: usize,
}

impl Worker {
    /// Create a new worker, with the specified maximum number of graph nodes,
    /// and set up communication channels.
    pub fn create(max_size: usize) -> (Worker, Sender<Node>, Receiver<Node>) {
        let (tx, to_worker) = Queue::new();
        let (from_worker, rx) = Queue::new();
        let graph = Graph::new(max_size);
        let worker = Worker {
            to_worker: to_worker,
            from_worker: from_worker,
            graph: graph,
            root: 0,
        };
        (worker, tx, rx)
    }

    /// Process a node. In normal operation, nodes are sent to the queue,
    /// but this function is available to initialize the graph into a good
    /// state before starting any work. Allocates.
    pub fn handle_node(&mut self, node: Node) {
        self.handle_item(Item::make_item(node));
    }

    fn handle_item(&mut self, item: Item<Node>) {
        let ix = item.ix;
        let old_item = self.graph.replace(ix, Some(item));
        if let Some(old_item) = old_item {
            self.from_worker.send_item(old_item);
        }        
    }

    /// Process the incoming items, run the graph, and return the rendered audio
    /// buffers. Lock-free.
    pub fn work(&mut self) -> &[Buffer] {
        for item in self.to_worker.recv_items() {
            self.handle_item(item);
        }
        self.graph.run_graph(self.root);
        self.graph.get_out_bufs(self.root)
    }
}
