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

use queue::{Receiver, Sender};
use graph::{Node, Message, SetParam, Note};
use modules;
use time;

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

/// The core owns the connection to the real-time worker.
struct Core {
    sample_rate: f32,
    rx: Receiver<Message>,
    tx: Sender<Message>,
}

#[derive(Clone)]
pub struct NoteEvent {
    pub down: bool,
    pub note: u8,
    pub velocity: u8,
}

struct Midi {
    cur_note: Option<u8>,
}

impl Engine {
    pub fn new(sample_rate: f32, rx: Receiver<Message>, tx: Sender<Message>) -> Engine {
        let core = Core::new(sample_rate, rx, tx);
        Engine { core, midi: None }
    }

    /// Initialize the engine with a simple mono synth.
    pub fn init_monosynth(&mut self) {
        self.core.init_monosynth();
        self.midi = Some(Midi::new());
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

    pub fn poll_rx(&mut self) -> usize {
        self.core.poll_rx()
    }
}

impl Core {
    fn new(sample_rate: f32, rx: Receiver<Message>, tx: Sender<Message>) -> Core {
        Core { sample_rate, rx, tx }
    }

    fn init_monosynth(&mut self) {
        let module = Box::new(modules::Saw::new(self.sample_rate));
        self.send_node(Node::create(module, 1, [], [(5, 0)]));
        let module = Box::new(modules::SmoothCtrl::new(880.0f32.log2()));
        self.send_node(Node::create(module, 3, [], []));
        let module = Box::new(modules::SmoothCtrl::new(0.5));
        self.send_node(Node::create(module, 4, [], []));
        let module = Box::new(modules::NotePitch::new());
        self.send_node(Node::create(module, 5, [], []));
        let module = Box::new(modules::Biquad::new(self.sample_rate));
        self.send_node(Node::create(module, 6, [(1,0)], [(3, 0), (4, 0)]));
        let module = Box::new(modules::Adsr::new());
        self.send_node(Node::create(module, 7, [], vec![(11, 0), (12, 0), (13, 0), (14, 0)]));
        let module = Box::new(modules::Gain::new());
        self.send_node(Node::create(module, 0, [(6, 0)], [(7, 0)]));

        let module = Box::new(modules::SmoothCtrl::new(5.0));
        self.send_node(Node::create(module, 11, [], []));
        let module = Box::new(modules::SmoothCtrl::new(5.0));
        self.send_node(Node::create(module, 12, [], []));
        let module = Box::new(modules::SmoothCtrl::new(4.0));
        self.send_node(Node::create(module, 13, [], []));
        let module = Box::new(modules::SmoothCtrl::new(5.0));
        self.send_node(Node::create(module, 14, [], []));
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
}

impl Midi {
    fn new() -> Midi {
        Midi {
            cur_note: None,
        }
    }

    fn set_ctrl_const(&mut self, core: &mut Core, value: u8, lo: f32, hi: f32, ix: usize,
        ts: u64)
    {
        let value = lo + value as f32 * (1.0/127.0) * (hi - lo);
        let param = SetParam {
            ix: ix,
            param_ix: 0,
            val: value,
            timestamp: ts,
        };
        core.send(Message::SetParam(param));
    }

    fn send_note(&mut self, core: &mut Core, ixs: Vec<usize>, midi_num: f32, velocity: f32,
        on: bool, ts: u64)
    {
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
                    1 => self.set_ctrl_const(core, value, 0.0, 22_000f32.log2(), 3, ts),
                    2 => self.set_ctrl_const(core, value, 0.0, 0.995, 4, ts),
                    3 => self.set_ctrl_const(core, value, 0.0, 22_000f32.log2(), 5, ts),

                    5 => self.set_ctrl_const(core, value, 0.0, 10.0, 11, ts),
                    6 => self.set_ctrl_const(core, value, 0.0, 10.0, 12, ts),
                    7 => self.set_ctrl_const(core, value, 0.0, 6.0, 13, ts),
                    8 => self.set_ctrl_const(core, value, 0.0, 10.0, 14, ts),
                    _ => println!("don't have handler for controller {}", controller),
                }
                i += 3;
            } else if data[i] == 0x90 || data[i] == 0x80 {
                let midi_num = data[i + 1];
                let velocity = data[i + 2];
                let on = data[i] == 0x90 && velocity > 0;
                if on || self.cur_note == Some(midi_num) {
                    self.send_note(core, vec![5, 7], midi_num as f32, velocity as f32, on, ts);
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
