// Copyright 2018 Google LLC
// 
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// 
//     https://www.apache.org/licenses/LICENSE-2.0
// 
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! The WebAssembly bindings for the synthesizer core.

#![feature(proc_macro, wasm_import_module, wasm_custom_section)]
extern crate wasm_bindgen;
extern crate synthesizer_io_core;
use wasm_bindgen::prelude::*;

use std::cell::RefCell;

use synthesizer_io_core::modules;

use synthesizer_io_core::worker::Worker;
use synthesizer_io_core::queue::{Receiver, Sender};
use synthesizer_io_core::graph::{Message, Node};
use synthesizer_io_core::module::N_SAMPLES_PER_CHUNK;

#[wasm_bindgen]
pub struct Synth {
    worker: Worker,
    tx: Sender<Message>,
    rx: Receiver<Message>,
}

#[wasm_bindgen]
impl Synth {
    pub fn new() -> Synth {
        let (worker, tx, rx) = Worker::create(1024);
        Synth { worker, tx, rx }
    }

    pub fn setup_saw(&mut self, val: f32) {
        let mut worker = &mut self.worker;
        let module = Box::new(modules::Saw::new(44_100.0));
        worker.handle_node(Node::create(module, 0, [], [(1, 0)]));
        let module = Box::new(modules::SmoothCtrl::new(val));
        worker.handle_node(Node::create(module, 1, [], []));
    }

    pub fn get_samples(&mut self, obuf: &mut[f32]) {
        let mut worker = &mut self.worker;
        let mut i = 0;
        let mut timestamp = 0;  // TODO: figure this out
        while i < obuf.len() {
            // should let the graph generate stereo
            let buf = worker.work(timestamp)[0].get();
            for j in 0..N_SAMPLES_PER_CHUNK {
                obuf[i + j] = buf[j];
            }
            timestamp += 1451247;  // 64 * 1e9 / 44_100
            i += N_SAMPLES_PER_CHUNK;
        }
    }
}
