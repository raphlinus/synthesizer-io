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

//! Synthesizer state and plumbing to UI.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use union_find::{QuickUnionUf, UnionByRank, UnionFind};

use druid::{HandlerCtx, Id, Ui, Widget};

use synthesizer_io_core::engine::{Engine, ModuleType, NoteEvent};

use crate::grid::{Delta, ModuleGrid, ModuleInstance, WireDelta, WireGrid};

/// Synthesizer engine state.
///
/// This struct owns the actual engine, and processes updates from the UI.
///
/// It is placed in the UI as a widget so that listeners can synchronously
/// access its state.
pub struct SynthState {
    // We probably want to move to the synth state fully owning the engine, and
    // things like midi being routed through the synth state. But for now this
    // should work pretty well.
    engine: Arc<Mutex<Engine>>,

    // Map grid coordinates to union-find node.
    coord_to_node: HashMap<(u16, u16), usize>,

    // Map from grid location of output pin to engine id.
    outputs: HashMap<(u16, u16), usize>,

    grid: WireGrid,

    // This might not be needed, we keep track of outputs already.
    modules: ModuleGrid,

    uf: QuickUnionUf<UnionByRank>,
}

#[derive(Clone)]
pub enum Action {
    Note(NoteEvent),
    Patch(Vec<Delta>),
    Poll(Vec<f32>),
}

impl Widget for SynthState {
    fn poke(&mut self, payload: &mut Any, _ctx: &mut HandlerCtx) -> bool {
        if let Some(action) = payload.downcast_mut::<Action>() {
            self.action(action);
            true
        } else {
            false
        }
    }
}

impl SynthState {
    pub fn new(engine: Arc<Mutex<Engine>>) -> SynthState {
        SynthState {
            engine,
            coord_to_node: HashMap::new(),
            outputs: HashMap::new(),
            grid: Default::default(),
            modules: Default::default(),
            uf: QuickUnionUf::new(0),
        }
    }

    pub fn ui(self, child: Id, ctx: &mut Ui) -> Id {
        ctx.add(self, &[child])
    }

    fn action(&mut self, action: &mut Action) {
        match *action {
            Action::Note(ref note_event) => {
                let mut engine = self.engine.lock().unwrap();
                engine.dispatch_note_event(note_event);
            }
            Action::Patch(ref delta) => self.apply_patch_delta(delta),
            Action::Poll(ref mut samples) => {
                let mut engine = self.engine.lock().unwrap();
                let _n_msg = engine.poll_rx();
                *samples = engine.poll_monitor();
            }
        }
    }

    fn apply_patch_delta(&mut self, delta: &[Delta]) {
        for d in delta {
            match d {
                Delta::Wire(WireDelta { grid_ix, val }) => {
                    self.grid.set(*grid_ix, *val);
                    self.update_wiring();
                }
                Delta::Jumper(delta) => {
                    self.grid.apply_jumper_delta(delta.clone());
                    self.update_wiring();
                }
                Delta::Module(inst) => {
                    self.add_module(inst);
                }
            }
        }
    }

    fn add_module(&mut self, inst: &ModuleInstance) {
        self.modules.add(inst.clone());
        let output_pin_coords = ModuleGrid::determine_output_pin(inst);
        let mut engine = self.engine.lock().unwrap();
        let module_type = match inst.spec.name.as_str() {
            "sin" => ModuleType::Sin,
            "saw" => ModuleType::Saw,
            _ => ModuleType::Sin, // just to do something
        };
        let ll_id = engine.instantiate_module(0, module_type);
        self.outputs.insert(output_pin_coords, ll_id);
    }

    // Return uf node.
    fn find_node(&mut self, coords: (u16, u16)) -> usize {
        let uf = &mut self.uf;
        *self
            .coord_to_node
            .entry(coords)
            .or_insert_with(|| uf.insert(Default::default()))
    }

    fn update_wiring(&mut self) {
        self.recompute_wire_net();

        let output_uf = self.find_node((19, 15));
        let output_uf = self.uf.find(output_uf);

        let mut output_bus = Vec::new();
        // Make borrow checker happy :/
        let outputs_clone = self.outputs.clone();
        for ((i, j), node) in &outputs_clone {
            let uf = self.find_node((*i, *j));
            let uf = self.uf.find(uf);
            if uf == output_uf {
                output_bus.push(*node);
            }
        }

        let mut engine = self.engine.lock().unwrap();
        engine.set_outputs(&output_bus);
    }

    fn recompute_wire_net(&mut self) {
        // Always recompute new net from scratch; maybe more incremental later.
        self.uf = QuickUnionUf::new(0);
        self.coord_to_node.clear();
        // TODO: this is just to make the borrow checker happy, can refactor.
        let grid_clone = self.grid.iter().cloned().collect::<Vec<_>>();
        for (i, j, is_vert) in &grid_clone {
            let node0 = self.find_node((*i, *j));
            let coords1 = if *is_vert { (*i, j + 1) } else { (i + 1, *j) };
            let node1 = self.find_node(coords1);
            self.uf.union(node0, node1);
        }

        let jumper_clone = self.grid.iter_jumpers().cloned().collect::<Vec<_>>();
        for (i0, j0, i1, j1) in &jumper_clone {
            let node0 = self.find_node((*i0, *j0));
            let node1 = self.find_node((*i1, *j1));
            self.uf.union(node0, node1);
        }
    }
}
