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

//! A graph runner that avoids all blocking operations, suitable for realtime threads.

use std::ops::DerefMut;
use std::ptr;
use std::mem;

use queue::Item;
use module::{Module, Buffer};


// maximum number of control inputs
const MAX_CTRL: usize = 16;

// maximum number of buffer inputs
const MAX_BUF: usize = 16;

pub const SENTINEL: usize = !0;

pub struct Graph {
    nodes: Box<[Option<Item<Node>>]>,

    // state for topo sort; all have same len
    visited: Box<[VisitedState]>,
    // TODO: there's no compelling reason for the graph sorting
    // to use linked lists, we could just use one vector for the
    // stack and another for the result.
    link: Box<[usize]>,
}

#[derive(Copy, Clone, PartialEq)]
enum VisitedState {
    NotVisited,
    Pushed,
    Scanned,
}

use self::VisitedState::*;

pub struct Node {
    pub ix: usize,

    module: Box<Module>,
    // module ix and index within its out_buf slice
    in_buf_wiring: Box<[(usize, usize)]>,
    // module ix and index within its out_ctrl slice
    in_ctrl_wiring: Box<[(usize, usize)]>,
    out_bufs: Box<[Buffer]>,
    out_ctrl: Box<[f32]>,
}

impl Node {
    /// Create a new node. The index must be given, as well as the input wiring.
    // TODO: should we take Vec instead of boxed slice?
    pub fn create(module: Box<Module>, ix: usize, in_buf_wiring: Box<[(usize, usize)]>,
        in_ctrl_wiring: Box<[(usize, usize)]>) -> Node
    {
        let n_bufs = module.n_bufs_out();
        let mut out_bufs = Vec::with_capacity(n_bufs);
        for _ in 0..n_bufs {
            out_bufs.push(Buffer::default());
        }
        let out_bufs = out_bufs.into_boxed_slice();
        let out_ctrl = vec![0.0; module.n_bufs_out()].into_boxed_slice();
        Node {
            ix: ix,
            module: module,
            in_buf_wiring: in_buf_wiring,
            in_ctrl_wiring: in_ctrl_wiring,
            out_bufs: out_bufs,
            out_ctrl: out_ctrl,
        }
    }
}

impl Graph {
    pub fn new(max_size: usize) -> Graph {
        // can't use vec! for nodes because Item isn't Clone
        let mut nodes = Vec::with_capacity(max_size);
        for _ in 0..max_size {
            nodes.push(None);
        }
        Graph {
            nodes: nodes.into_boxed_slice(),
            visited: vec![NotVisited; max_size].into_boxed_slice(),
            link: vec![0; max_size].into_boxed_slice(),
        }
    }

    /// Get the output buffers for the specified graph node. Panics if the
    /// index is not a valid, populated node. Lock-free.
    pub fn get_out_bufs(&self, ix: usize) -> &[Buffer] {
        &self.nodes[ix].as_ref().unwrap().out_bufs
    }

    /// Replace a graph node with a new item, returning the old value.
    /// Lock-free.
    pub fn replace(&mut self, ix: usize, item: Option<Item<Node>>) -> Option<Item<Node>> {
        mem::replace(&mut self.nodes[ix], item)
    }

    fn run_one_module(&mut self, module_ix: usize, ctrl: &mut [f32; MAX_CTRL],
        bufs: &mut [*const Buffer; MAX_BUF])
    {
        {
            let this = self.nodes[module_ix].as_ref().unwrap();
            for (i, &(mod_ix, buf_ix)) in this.in_buf_wiring.iter().enumerate() {
                // otherwise the transmute would cause aliasing
                assert!(module_ix != mod_ix);
                bufs[i] = &self.get_out_bufs(mod_ix)[buf_ix];
            }
            for (i, &(mod_ix, ctrl_ix)) in this.in_ctrl_wiring.iter().enumerate() {
                ctrl[i] = self.nodes[mod_ix].as_ref().unwrap().out_ctrl[ctrl_ix];
            }
        }
        let this = self.nodes[module_ix].as_mut().unwrap().deref_mut();
        let buf_in = unsafe { mem::transmute(&bufs[..this.in_buf_wiring.len()]) };
        let ctrl_in = &ctrl[..this.in_ctrl_wiring.len()];
        this.module.process(ctrl_in, &mut this.out_ctrl, buf_in, &mut this.out_bufs);
    }

    fn topo_sort(&mut self, root: usize) -> usize {
        // initially the result linked list is empty
        let mut head = SENTINEL;
        let mut tail = SENTINEL;

        // initially the stack just contains root
        self.link[root] = SENTINEL;
        let mut stack = root;
        self.visited[root] = Pushed;

        while stack != SENTINEL {
            if self.visited[stack] == Pushed {
                self.visited[stack] = Scanned;
                let node = self.nodes[stack].as_ref().unwrap();
                for &(ix, _) in node.in_buf_wiring.iter().chain(node.in_ctrl_wiring.iter()) {
                    if self.visited[ix] == NotVisited {
                        self.visited[ix] = Pushed;
                        // push ix on stack
                        self.link[ix] = stack;
                        stack = ix;
                    }
                }
            }
            if self.visited[stack] == Scanned {
                let next = self.link[stack];

                // add `stack` to end of result linked list
                self.link[stack] = SENTINEL;
                if head == SENTINEL {
                    head = stack;
                }
                if tail != SENTINEL {
                    self.link[tail] = stack;
                }
                tail = stack;

                // pop stack
                stack = next;
            }
        }
        head
    }

    /// Run the graph. On return, the buffer for the given root node will be
    /// filled. Designed to be lock-free.
    pub fn run_graph(&mut self, root: usize) {
        // scratch space, here to amortize the initialization costs
        let mut ctrl = [0.0f32; MAX_CTRL];
        let mut bufs = [ptr::null(); MAX_BUF];

        // TODO: don't do topo sort every time, reuse if graph hasn't changed
        let mut ix = self.topo_sort(root);
        while ix != SENTINEL {
            self.run_one_module(ix, &mut ctrl, &mut bufs);
            self.visited[ix] = NotVisited;  // reset state for next topo sort
            ix = self.link[ix];
        }
    }
}
