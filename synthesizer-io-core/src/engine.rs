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

//! Interface for the audio engine.

use time;

use crate::graph::{IntoBoxedSlice, Message, Node, Note, SetParam};
use crate::id_allocator::IdAllocator;
use crate::module::Module;
use crate::modules;
use crate::queue::{Receiver, Sender};

/// The interface from the application to the audio engine.
///
/// It doesn't do the synthesis itself; the Worker (running in a real time
/// thread) handles that, but this module is responsible for driving
/// that process by sending messages.
pub struct Engine {
    core: Core,

    // We have a midi state in the engine, but this may get factored out.
    midi: Option<Midi>,
}

/// Type used to identify nodes in the external interface (not to be confused
/// with nodes in the low-level graph).
pub type NodeId = usize;

/// The type of a module to be instantiated. It's not clear this should be
/// an enum, but it should do for now.
pub enum ModuleType {
    Sin,
    Saw,
}

/// The core owns the connection to the real-time worker.
struct Core {
    sample_rate: f32,
    rx: Receiver<Message>,
    tx: Sender<Message>,

    id_alloc: IdAllocator,

    monitor_queues: Option<MonitorQueues>,
}

#[derive(Clone)]
pub struct NoteEvent {
    pub down: bool,
    pub note: u8,
    pub velocity: u8,
}

struct Midi {
    control_map: ControlMap,
    cur_note: Option<u8>,
}

struct ControlMap {
    cutoff: usize,
    reso: usize,

    attack: usize,
    decay: usize,
    sustain: usize,
    release: usize,

    // node number of node that can be replaced to inject more audio
    ext: usize,

    note_receivers: Vec<usize>,
}

struct MonitorQueues {
    rx: Receiver<Vec<f32>>,
    tx: Sender<Vec<f32>>,
}

impl Engine {
    /// Create a new engine instance.
    ///
    /// This call takes ownership of channels to and from the worker.
    pub fn new(sample_rate: f32, rx: Receiver<Message>, tx: Sender<Message>) -> Engine {
        let core = Core::new(sample_rate, rx, tx);
        Engine { core, midi: None }
    }

    /// Initialize the engine with a simple mono synth.
    pub fn init_monosynth(&mut self) {
        let control_map = self.core.init_monosynth();
        self.midi = Some(Midi::new(control_map));
    }

    /// Handle a MIDI event.
    pub fn dispatch_midi(&mut self, data: &[u8], ts: u64) {
        if let Some(ref mut midi) = self.midi {
            midi.dispatch_midi(&mut self.core, data, ts);
        }
    }

    /// Handle a note event.
    pub fn dispatch_note_event(&mut self, note_event: &NoteEvent) {
        if let Some(ref mut midi) = self.midi {
            midi.dispatch_note_event(&mut self.core, note_event);
        }
    }

    /// Poll the return queue. Right now this just returns the number of items
    /// retrieved.
    pub fn poll_rx(&mut self) -> usize {
        self.core.poll_rx()
    }

    /// Poll the monitor queue, retrieving audio data.
    pub fn poll_monitor(&mut self) -> Vec<f32> {
        self.core.poll_monitor()
    }

    /// Instantiate a module. Right now, the module has no inputs and the output
    /// is run directly to the output bus, but we'll soon add the ability to
    /// manipulate a wiring graph.
    ///
    /// Returns an id for the module's output. (TODO: will obviously need work for
    /// multi-output modules)
    pub fn instantiate_module(&mut self, node_id: NodeId, ty: ModuleType) -> usize {
        self.core.instantiate_module(node_id, ty)
    }

    /// Set the output bus.
    pub fn set_outputs(&mut self, outputs: &[usize]) {
        let sum_node = match self.midi {
            Some(Midi {
                control_map: ControlMap { ext, .. },
                ..
            }) => ext,
            _ => 0,
        };
        self.core.update_sum_node(sum_node, outputs);
    }
}

impl Core {
    fn new(sample_rate: f32, rx: Receiver<Message>, tx: Sender<Message>) -> Core {
        let mut id_alloc = IdAllocator::new();
        id_alloc.reserve(0);
        let monitor_queues = None;
        Core {
            sample_rate,
            rx,
            tx,
            id_alloc,
            monitor_queues,
        }
    }

    pub fn create_node<
        B1: IntoBoxedSlice<(usize, usize)>,
        B2: IntoBoxedSlice<(usize, usize)>,
        M: Module + 'static,
    >(
        &mut self,
        module: M,
        in_buf_wiring: B1,
        in_ctrl_wiring: B2,
    ) -> usize {
        let id = self.id_alloc.alloc();
        self.send_node(Node::create(
            Box::new(module),
            id,
            in_buf_wiring,
            in_ctrl_wiring,
        ));
        id
    }

    fn init_monosynth(&mut self) -> ControlMap {
        let sample_rate = self.sample_rate;
        let note_pitch = self.create_node(modules::NotePitch::new(), [], []);
        let saw = self.create_node(modules::Saw::new(sample_rate), [], [(note_pitch, 0)]);
        let cutoff = self.create_node(modules::SmoothCtrl::new(880.0f32.log2()), [], []);
        let reso = self.create_node(modules::SmoothCtrl::new(0.5), [], []);
        let filter_out = self.create_node(
            modules::Biquad::new(sample_rate),
            [(saw, 0)],
            [(cutoff, 0), (reso, 0)],
        );

        let attack = self.create_node(modules::SmoothCtrl::new(5.0), [], []);
        let decay = self.create_node(modules::SmoothCtrl::new(5.0), [], []);
        let sustain = self.create_node(modules::SmoothCtrl::new(4.0), [], []);
        let release = self.create_node(modules::SmoothCtrl::new(5.0), [], []);
        let adsr = self.create_node(
            modules::Adsr::new(),
            [],
            vec![(attack, 0), (decay, 0), (sustain, 0), (release, 0)],
        );
        let env_out = self.create_node(modules::Gain::new(), [(filter_out, 0)], [(adsr, 0)]);

        let ext = self.create_node(modules::Sum::new(), [], []);
        let ext_gain = self.create_node(modules::ConstCtrl::new(-2.0), [], []);
        let ext_atten = self.create_node(modules::Gain::new(), [(ext, 0)], [(ext_gain, 0)]);

        let monitor_in = self.create_node(modules::Sum::new(), [(env_out, 0), (ext_atten, 0)], []);

        let (monitor, tx, rx) = modules::Monitor::new();
        self.monitor_queues = Some(MonitorQueues { tx, rx });
        let monitor = self.create_node(monitor, [(monitor_in, 0)], []);

        self.update_sum_node(0, &[monitor]);

        ControlMap {
            cutoff,
            reso,
            attack,
            decay,
            sustain,
            release,
            ext,
            note_receivers: vec![note_pitch, adsr],
        }
    }

    fn send(&self, msg: Message) {
        self.tx.send(msg);
    }

    fn send_node(&mut self, node: Node) {
        self.send(Message::Node(node));
    }

    fn poll_rx(&mut self) -> usize {
        self.rx.recv().count()
    }

    fn poll_monitor(&self) -> Vec<f32> {
        let mut result = Vec::new();
        if let Some(ref qs) = self.monitor_queues {
            for mut item in qs.rx.recv_items() {
                result.extend_from_slice(&item);
                item.clear();
                qs.tx.send_item(item);
            }
        }
        result
    }

    fn update_sum_node(&mut self, sum_node: usize, outputs: &[usize]) {
        let module = Box::new(modules::Sum::new());
        let buf_wiring: Vec<_> = outputs.iter().map(|n| (*n, 0)).collect();
        self.send_node(Node::create(module, sum_node, buf_wiring, []));
    }

    fn instantiate_module(&mut self, _node_id: NodeId, ty: ModuleType) -> usize {
        let ll_id = match ty {
            ModuleType::Sin => {
                let pitch = self.create_node(modules::SmoothCtrl::new(440.0f32.log2()), [], []);
                let sample_rate = self.sample_rate;
                self.create_node(modules::Sin::new(sample_rate), [], [(pitch, 0)])
            }
            ModuleType::Saw => {
                let pitch = self.create_node(modules::SmoothCtrl::new(440.0f32.log2()), [], []);
                let sample_rate = self.sample_rate;
                self.create_node(modules::Saw::new(sample_rate), [], [(pitch, 0)])
            }
        };
        ll_id
    }
}

impl Midi {
    fn new(control_map: ControlMap) -> Midi {
        Midi {
            control_map,
            cur_note: None,
        }
    }

    fn set_ctrl_const(&mut self, core: &mut Core, value: u8, lo: f32, hi: f32, ix: usize, ts: u64) {
        let value = lo + value as f32 * (1.0 / 127.0) * (hi - lo);
        let param = SetParam {
            ix: ix,
            param_ix: 0,
            val: value,
            timestamp: ts,
        };
        core.send(Message::SetParam(param));
    }

    fn send_note(
        &mut self,
        core: &mut Core,
        ixs: Vec<usize>,
        midi_num: f32,
        velocity: f32,
        on: bool,
        ts: u64,
    ) {
        let note = Note {
            ixs: ixs.into_boxed_slice(),
            midi_num: midi_num,
            velocity: velocity,
            on: on,
            timestamp: ts,
        };
        core.send(Message::Note(note));
    }

    fn dispatch_midi(&mut self, core: &mut Core, data: &[u8], ts: u64) {
        let mut i = 0;
        while i < data.len() {
            if data[i] == 0xb0 {
                let controller = data[i + 1];
                let value = data[i + 2];
                match controller {
                    1 => {
                        let cutoff = self.control_map.cutoff;
                        self.set_ctrl_const(core, value, 0.0, 22_000f32.log2(), cutoff, ts);
                    }
                    2 => {
                        let reso = self.control_map.reso;
                        self.set_ctrl_const(core, value, 0.0, 0.995, reso, ts);
                    }

                    5 => {
                        let attack = self.control_map.attack;
                        self.set_ctrl_const(core, value, 0.0, 10.0, attack, ts);
                    }
                    6 => {
                        let decay = self.control_map.decay;
                        self.set_ctrl_const(core, value, 0.0, 10.0, decay, ts);
                    }
                    7 => {
                        let sustain = self.control_map.sustain;
                        self.set_ctrl_const(core, value, 0.0, 6.0, sustain, ts);
                    }
                    8 => {
                        let release = self.control_map.release;
                        self.set_ctrl_const(core, value, 0.0, 10.0, release, ts);
                    }
                    _ => println!("don't have handler for controller {}", controller),
                }
                i += 3;
            } else if data[i] == 0x90 || data[i] == 0x80 {
                let midi_num = data[i + 1];
                let velocity = data[i + 2];
                let on = data[i] == 0x90 && velocity > 0;
                if on || self.cur_note == Some(midi_num) {
                    let targets = self.control_map.note_receivers.clone();
                    self.send_note(core, targets, midi_num as f32, velocity as f32, on, ts);
                    self.cur_note = if on { Some(midi_num) } else { None }
                }
                i += 3;
            } else {
                break;
            }
        }
    }

    fn dispatch_note_event(&mut self, core: &mut Core, note_event: &NoteEvent) {
        let mut data = [0u8; 3];
        data[0] = if note_event.down { 0x90 } else { 0x80 };
        data[1] = note_event.note;
        data[2] = note_event.velocity;
        self.dispatch_midi(core, &data, time::precise_time_ns());
    }
}
