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

//! Windows GUI music synthesizer app.

extern crate cpal;
extern crate midir;
extern crate direct2d;
extern crate xi_win_ui;
extern crate xi_win_shell;
extern crate synthesizer_io_core;
extern crate time;

mod ui;

use std::ops::DerefMut;
use std::thread;

use cpal::{EventLoop, StreamData, UnknownTypeOutputBuffer};
use midir::{MidiInput, MidiInputConnection};

use synthesizer_io_core::modules;

use synthesizer_io_core::worker::Worker;
use synthesizer_io_core::queue::Sender;
use synthesizer_io_core::graph::{Node, Message, SetParam, Note};
use synthesizer_io_core::module::N_SAMPLES_PER_CHUNK;

use xi_win_shell::win_main;
use xi_win_shell::window::WindowBuilder;

use xi_win_ui::{UiMain, UiState};
use xi_win_ui::widget::{Button, Column, EventForwarder};

use ui::{NoteEvent, Piano};

// This is cut'n'paste; we'll both continue developing it and factor things out across
// the various modules.
struct Midi {
    tx: Sender<Message>,
    cur_note: Option<u8>,
}

struct SynthState {
    midi: Midi,
}

impl Midi {
    fn new(tx: Sender<Message>) -> Midi {
        Midi {
            tx: tx,
            cur_note: None,
        }
    }

    fn send(&self, msg: Message) {
        self.tx.send(msg);
    }

    fn set_ctrl_const(&mut self, value: u8, lo: f32, hi: f32, ix: usize, ts: u64) {
        let value = lo + value as f32 * (1.0/127.0) * (hi - lo);
        let param = SetParam {
            ix: ix,
            param_ix: 0,
            val: value,
            timestamp: ts,
        };
        self.send(Message::SetParam(param));
    }

    fn send_note(&mut self, ixs: Vec<usize>, midi_num: f32, velocity: f32, on: bool,
        ts: u64)
    {
        let note = Note {
            ixs: ixs.into_boxed_slice(),
            midi_num: midi_num,
            velocity: velocity,
            on: on,
            timestamp: ts,
        };
        self.send(Message::Note(note));
    }

    fn dispatch_midi(&mut self, data: &[u8], ts: u64) {
        let mut i = 0;
        while i < data.len() {
            if data[i] == 0xb0 {
                let controller = data[i + 1];
                let value = data[i + 2];
                match controller {
                    1 => self.set_ctrl_const(value, 0.0, 22_000f32.log2(), 3, ts),
                    2 => self.set_ctrl_const(value, 0.0, 0.995, 4, ts),
                    3 => self.set_ctrl_const(value, 0.0, 22_000f32.log2(), 5, ts),

                    5 => self.set_ctrl_const(value, 0.0, 10.0, 11, ts),
                    6 => self.set_ctrl_const(value, 0.0, 10.0, 12, ts),
                    7 => self.set_ctrl_const(value, 0.0, 6.0, 13, ts),
                    8 => self.set_ctrl_const(value, 0.0, 10.0, 14, ts),
                    _ => println!("don't have handler for controller {}", controller),
                }
                i += 3;
            } else if data[i] == 0x90 || data[i] == 0x80 {
                let midi_num = data[i + 1];
                let velocity = data[i + 2];
                let on = data[i] == 0x90 && velocity > 0;
                if on || self.cur_note == Some(midi_num) {
                    self.send_note(vec![5, 7], midi_num as f32, velocity as f32, on, ts);
                    self.cur_note = if on { Some(midi_num) } else { None }
                }
                i += 3;
            } else {
                break;
            }
        }
    }

    fn dispatch_note_event(&mut self, note_event: &NoteEvent) {
        let mut data = [0u8; 3];
        data[0] = if note_event.down { 0x90 } else { 0x80 };
        data[1] = note_event.note;
        data[2] = note_event.velocity;
        self.dispatch_midi(&data, time::precise_time_ns());
    }
}

impl SynthState {
    fn action(&mut self, note_event: &NoteEvent) {
        self.midi.dispatch_note_event(note_event);
    }
}

fn main() {
    xi_win_shell::init();
    let (mut worker, tx, rx) = Worker::create(1024);
    let mut synth_state = SynthState { midi: Midi::new(tx.clone()) };

    let module = Box::new(modules::Saw::new(44_100.0));
    worker.handle_node(Node::create(module, 1, [], [(5, 0)]));
    let module = Box::new(modules::SmoothCtrl::new(880.0f32.log2()));
    worker.handle_node(Node::create(module, 3, [], []));
    let module = Box::new(modules::SmoothCtrl::new(0.5));
    worker.handle_node(Node::create(module, 4, [], []));
    let module = Box::new(modules::NotePitch::new());
    worker.handle_node(Node::create(module, 5, [], []));
    let module = Box::new(modules::Biquad::new(44_100.0));
    worker.handle_node(Node::create(module, 6, [(1,0)], [(3, 0), (4, 0)]));
    let module = Box::new(modules::Adsr::new());
    worker.handle_node(Node::create(module, 7, [], vec![(11, 0), (12, 0), (13, 0), (14, 0)]));
    let module = Box::new(modules::Gain::new());
    worker.handle_node(Node::create(module, 0, [(6, 0)], [(7, 0)]));

    let module = Box::new(modules::SmoothCtrl::new(5.0));
    worker.handle_node(Node::create(module, 11, [], []));
    let module = Box::new(modules::SmoothCtrl::new(5.0));
    worker.handle_node(Node::create(module, 12, [], []));
    let module = Box::new(modules::SmoothCtrl::new(4.0));
    worker.handle_node(Node::create(module, 13, [], []));
    let module = Box::new(modules::SmoothCtrl::new(5.0));
    worker.handle_node(Node::create(module, 14, [], []));

    let mut run_loop = win_main::RunLoop::new();
    let mut builder = WindowBuilder::new();
    let mut state = UiState::new();
    let button = Button::new("Press me").ui(&mut state);
    let piano = Piano::new().ui(&mut state);
    let column = Column::new().ui(&[button, piano], &mut state);
    let forwarder = EventForwarder::<NoteEvent>::new().ui(column, &mut state);
    state.add_listener(piano, move |event: &mut NoteEvent, mut ctx| {
        ctx.poke_up(event);
    });
    state.add_listener(forwarder, move |action: &mut NoteEvent, _ctx| {
        synth_state.action(action);
    });
    state.set_root(forwarder);
    builder.set_handler(Box::new(UiMain::new(state)));
    builder.set_title("Synthesizer IO");
    let window = builder.build().unwrap();
    let _midi_connection = setup_midi(tx);  // keep from being dropped
    thread::spawn(move || run_cpal(worker));
    window.show();
    run_loop.run();
}

fn setup_midi(tx: Sender<Message>) -> Option<MidiInputConnection<()>> {
    let mut midi = Midi::new(tx);

    let mut midi_in = MidiInput::new("midir input").expect("can't create midi input");
    midi_in.ignore(::midir::Ignore::None);
    let result = midi_in.connect(0, "in", move |_ts, data, _| {
        //println!("{}, {:?}", ts, data);
        midi.dispatch_midi(data, time::precise_time_ns());
    }, ());
    if let Err(ref e) = result {
        println!("error connecting to midi: {:?}", e);
    }
    result.ok()
}

fn run_cpal(mut worker: Worker) {
    let event_loop = EventLoop::new();
    let device = cpal::default_output_device().expect("no output device");
    let mut supported_formats_range = device.supported_output_formats()
        .expect("error while querying formats");
    let format = supported_formats_range.next()
        .expect("no supported format?!")
        .with_max_sample_rate();
    println!("format: {:?}", format);
    let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
    event_loop.play_stream(stream_id);

    event_loop.run(move |_stream_id, stream_data| {
        match stream_data {
            StreamData::Output { buffer: UnknownTypeOutputBuffer::F32(mut buf) } => {
                let mut buf_slice = buf.deref_mut();
                let mut i = 0;
                let mut timestamp = time::precise_time_ns();
                while i < buf_slice.len() {
                    // should let the graph generate stereo
                    let buf = worker.work(timestamp)[0].get();
                    for j in 0..N_SAMPLES_PER_CHUNK {
                        buf_slice[i + j * 2] = buf[j];
                        buf_slice[i + j * 2 + 1] = buf[j];
                    }

                    // TODO: calculate properly, magic value is 64 * 1e9 / 44_100
                    timestamp += 1451247 * (N_SAMPLES_PER_CHUNK as u64) / 64;
                    i += N_SAMPLES_PER_CHUNK * 2;
                }
            }
            _ => panic!("Can't handle output buffer format"),
        }
    });
}
