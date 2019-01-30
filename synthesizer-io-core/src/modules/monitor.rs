// Copyright 2018 The Synthesizer IO Authors.
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

//! A module for monitoring an audio signal.

use crate::module::{Buffer, Module};
use crate::queue::{Item, Queue, Receiver, Sender};

pub struct Monitor {
    buf_pool: Vec<Item<Vec<f32>>>,
    to_monitor: Receiver<Vec<f32>>,
    from_monitor: Sender<Vec<f32>>,
}

const POOL_SIZE: usize = 256;

const BUF_SIZE: usize = 256;

impl Monitor {
    pub fn new() -> (Monitor, Sender<Vec<f32>>, Receiver<Vec<f32>>) {
        let (tx, to_monitor) = Queue::new();
        let (from_monitor, rx) = Queue::new();
        let mut buf_pool = Vec::with_capacity(POOL_SIZE);
        for _ in 0..POOL_SIZE {
            buf_pool.push(Item::make_item(Vec::with_capacity(BUF_SIZE)));
        }
        let monitor = Monitor {
            buf_pool,
            to_monitor,
            from_monitor,
        };
        (monitor, tx, rx)
    }
}

impl Module for Monitor {
    fn n_bufs_out(&self) -> usize {
        1
    }

    fn process(
        &mut self,
        _control_in: &[f32],
        _control_out: &mut [f32],
        buf_in: &[&Buffer],
        buf_out: &mut [Buffer],
    ) {
        let cur_buf = self.buf_pool.pop();

        // Note: non-allocation depends on this not overflowing.
        self.buf_pool.extend(self.to_monitor.recv_items());

        let buf = buf_in[0].get();
        // Copy input to output. This is so node can participate in graph
        // topological sort, but maybe there's a better approach, like
        // having an explicit list of roots.
        buf_out[0].get_mut().copy_from_slice(buf);

        if let Some(mut cur_buf) = cur_buf {
            cur_buf.extend_from_slice(buf);
            if cur_buf.len() + buf.len() > cur_buf.capacity() {
                self.from_monitor.send_item(cur_buf);
            } else {
                self.buf_pool.push(cur_buf);
            }
        }
    }
}
