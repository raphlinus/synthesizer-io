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

const SENTINEL: usize = !0;

pub struct Graph {
    nodes: Box<[Option<Item<Message>>]>,

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

pub enum Message {
    Node(Node),
    SetParam(SetParam),
    Note(Note),
    Quit,
}

impl Message {
    fn get_node(&self) -> Option<&Node> {
        match *self {
            Message::Node(ref node) => Some(node),
            _ => None,
        }
    }
}

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

/// A struct that contains the data for setting a parameter
pub struct SetParam {
    pub ix: usize,
    pub param_ix: usize,
    pub val: f32,
    pub timestamp: u64,
}

/// A struct that represents a note on/off event
pub struct Note {
    pub ixs: Box<[usize]>,  // list of node ix's affected by this note
    pub midi_num: f32,  // 69.0 = A4 (440Hz)
    pub velocity: f32,  // 1 = minimum, 127 = maximum
    pub on: bool,
    pub timestamp: u64,
}

pub trait IntoBoxedSlice<T> {
    fn into_box(self) -> Box<[T]>;
}

impl<T> IntoBoxedSlice<T> for Vec<T> {
    fn into_box(self) -> Box<[T]> { self.into_boxed_slice() }
}

impl<T> IntoBoxedSlice<T> for Box<[T]> {
    fn into_box(self) -> Box<[T]> { self }
}

impl<'a, T: Clone> IntoBoxedSlice<T> for &'a [T] {
    fn into_box(self) -> Box<[T]> {
        let vec: Vec<T> = From::from(self);
        vec.into_boxed_slice()
    }
}

impl<T> IntoBoxedSlice<T> for [T; 0] {
    fn into_box(self) -> Box<[T]> { Vec::new().into_boxed_slice() }
}

impl<T: Clone> IntoBoxedSlice<T> for [T; 1] {
    fn into_box(self) -> Box<[T]> { self[..].into_box() }
}

impl<T: Clone> IntoBoxedSlice<T> for [T; 2] {
    fn into_box(self) -> Box<[T]> { self[..].into_box() }
}

impl Node {
    /// Create a new node. The index must be given, as well as the input wiring.
    pub fn create<B1: IntoBoxedSlice<(usize, usize)>,
                  B2: IntoBoxedSlice<(usize, usize)>>
        (module: Box<Module>, ix: usize, in_buf_wiring: B1, in_ctrl_wiring: B2) -> Node
    {
        let n_bufs = module.n_bufs_out();
        let mut out_bufs = Vec::with_capacity(n_bufs);
        for _ in 0..n_bufs {
            out_bufs.push(Buffer::default());
        }
        let out_bufs = out_bufs.into_boxed_slice();
        let out_ctrl = vec![0.0; module.n_ctrl_out()].into_boxed_slice();
        Node {
            ix: ix,
            module: module,
            in_buf_wiring: in_buf_wiring.into_box(),
            in_ctrl_wiring: in_ctrl_wiring.into_box(),
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
        &self.get_node(ix).unwrap().out_bufs
    }

    fn get_node(&self, ix: usize) -> Option<&Node> {
        self.nodes[ix].as_ref().and_then(|item| item.get_node())
    }

    fn get_node_mut(&mut self, ix: usize) -> Option<&mut Node> {
        self.nodes[ix].as_mut().and_then(|msg| match *msg.deref_mut() {
            Message::Node(ref mut n) => Some(n),
            _ => None
        })
    }

    pub fn get_module_mut(&mut self, ix: usize) -> &mut Module {
        self.get_node_mut(ix).unwrap().module.deref_mut()
    }

    /// Replace a graph node with a new item, returning the old value.
    /// Lock-free.
    pub fn replace(&mut self, ix: usize, item: Option<Item<Message>>) -> Option<Item<Message>> {
        let mut old_item = mem::replace(&mut self.nodes[ix], item);
        if let Some(ref mut old) = old_item {
            if let Message::Node(ref mut old_node) = *old.deref_mut() {
                self.get_node_mut(ix).unwrap().module.migrate(old_node.module.deref_mut());
            }
        }
        old_item
    }

    fn run_one_module(&mut self, module_ix: usize, ctrl: &mut [f32; MAX_CTRL],
        bufs: &mut [*const Buffer; MAX_BUF], timestamp: u64)
    {
        {
            let this = self.get_node(module_ix).unwrap();
            for (i, &(mod_ix, buf_ix)) in this.in_buf_wiring.iter().enumerate() {
                // otherwise the transmute would cause aliasing
                assert!(module_ix != mod_ix);
                bufs[i] = &self.get_out_bufs(mod_ix)[buf_ix];
            }
            for (i, &(mod_ix, ctrl_ix)) in this.in_ctrl_wiring.iter().enumerate() {
                ctrl[i] = self.get_node(mod_ix).unwrap().out_ctrl[ctrl_ix];
            }
        }
        let this = self.get_node_mut(module_ix).unwrap();
        let buf_in = unsafe { mem::transmute(&bufs[..this.in_buf_wiring.len()]) };
        let ctrl_in = &ctrl[..this.in_ctrl_wiring.len()];
        this.module.process_ts(ctrl_in, &mut this.out_ctrl, buf_in, &mut this.out_bufs,
            timestamp);
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
                let node = self.nodes[stack].as_ref().and_then(|item| item.get_node()).unwrap();
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
    pub fn run_graph(&mut self, root: usize, timestamp: u64) {
        // scratch space, here to amortize the initialization costs
        let mut ctrl = [0.0f32; MAX_CTRL];
        let mut bufs = [ptr::null(); MAX_BUF];

        // TODO: don't do topo sort every time, reuse if graph hasn't changed
        let mut ix = self.topo_sort(root);
        while ix != SENTINEL {
            self.run_one_module(ix, &mut ctrl, &mut bufs, timestamp);
            self.visited[ix] = NotVisited;  // reset state for next topo sort
            ix = self.link[ix];
        }
    }
}
