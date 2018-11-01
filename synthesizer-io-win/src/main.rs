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
extern crate directwrite;
extern crate xi_win_ui;
extern crate xi_win_shell;
extern crate synthesizer_io_core;
extern crate synthesize_scope;
extern crate time;
extern crate itertools;
extern crate winapi;
extern crate dxgi;

mod grid;
mod ui;

use std::any::Any;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::thread;

use cpal::{EventLoop, StreamData, UnknownTypeOutputBuffer};
use midir::{MidiInput, MidiInputConnection};

use synthesizer_io_core::modules;

use synthesizer_io_core::engine::{Engine, ModuleType, NoteEvent};
use synthesizer_io_core::worker::Worker;
use synthesizer_io_core::graph::Node;
use synthesizer_io_core::module::N_SAMPLES_PER_CHUNK;

use xi_win_shell::win_main;
use xi_win_shell::window::WindowBuilder;

use xi_win_ui::{HandlerCtx, Id, UiInner, UiMain, UiState, Widget};
use xi_win_ui::widget::{Button, Column, Label, Padding, Row};

use grid::{Delta, WireDelta};
use ui::{Patcher, PatcherAction, Piano, Scope, ScopeCommand};

/// Synthesizer engine state. This is placed in the UI as a widget so that
/// listeners can synchronously access its state.
struct SynthState {
    // We probably want to move to the synth state fully owning the engine, and
    // things like midi being routed through the synth state. But for now this
    // should work pretty well.
    engine: Arc<Mutex<Engine>>,
}

#[derive(Clone)]
enum Action {
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
    pub fn ui(self, child: Id, ctx: &mut UiInner) -> Id {
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
                    println!("got wire delta {:?} {}", grid_ix, val);
                }
                Delta::Module(_inst) => {
                    let mut engine = self.engine.lock().unwrap();
                    engine.instantiate_module(0, ModuleType::Sin);
                }
            }
        }
    }
}

fn padded_flex_row(children: &[Id], ui: &mut UiState) -> Id {
    let vec = children.iter().map(|&child|
        Padding::uniform(5.0).ui(child, ui)).collect::<Vec<_>>();
    let mut row = Row::new();
    for &child in &vec {
        row.set_flex(child, 1.0);
    }
    row.ui(&vec, ui)
}

/// Build the main window UI
fn build_ui(synth_state: SynthState, ui: &mut UiState) -> Id {
    let button = Label::new("Synthesizer IO").ui(ui);
    let patcher = Patcher::new().ui(ui);
    let scope = Scope::new().ui(ui);
    let piano = Piano::new().ui(ui);

    let modules = &["sine", "control", "saw", "biquad", "adsr", "gain"];

    let wire_b = Button::new("wire").ui(ui);
    ui.add_listener(wire_b, move |_: &mut bool, mut ctx| {
        ctx.poke(patcher, &mut PatcherAction::WireMode);
    });
    let mut buttons = vec![wire_b];
    for &module in modules {
        let button = Button::new(module).ui(ui);
        ui.add_listener(button, move |_: &mut bool, mut ctx| {
            ctx.poke(patcher, &mut PatcherAction::Module(module.into()));
        });        
        buttons.push(button);
    }
    let button_row = padded_flex_row(&buttons, ui);
    let mut column = Column::new();
    column.set_flex(patcher, 2.0);
    column.set_flex(scope, 2.0);
    column.set_flex(piano, 1.0);
    let column = column.ui(&[button, patcher, button_row, scope, piano], ui);
    let synth_state = synth_state.ui(column, ui);
    ui.add_listener(patcher, move |delta: &mut Vec<Delta>, mut ctx| {
        ctx.poke_up(&mut Action::Patch(delta.clone()));
    });
    ui.add_listener(scope, move |_event: &mut (), mut ctx| {
        let mut action = Action::Poll(Default::default());
        ctx.poke_up(&mut action);
        if let Action::Poll(samples) = action {
            ctx.poke(scope, &mut ScopeCommand::Samples(samples));
            //println!("polled {} events", _n_msg);
        }
    });
    ui.add_listener(piano, move |event: &mut NoteEvent, mut ctx| {
        ctx.poke_up(&mut Action::Note(event.clone()));
    });
    synth_state
}

fn main() {
    xi_win_shell::init();
    let (mut worker, tx, rx) = Worker::create(1024);
    // TODO: get sample rate from cpal
    let mut engine = Engine::new(48_000.0, rx, tx);
    engine.init_monosynth();

    let engine = Arc::new(Mutex::new(engine));

    let synth_state = SynthState { engine: engine.clone() };

    // Set up working graph; will probably be replaced by the engine before
    // the first audio callback runs.
    let module = Box::new(modules::Sum::new());
    worker.handle_node(Node::create(module, 0, [], []));

    let mut run_loop = win_main::RunLoop::new();
    let mut builder = WindowBuilder::new();
    let mut state = UiState::new();
    let root = build_ui(synth_state, &mut state);
    state.set_root(root);
    builder.set_handler(Box::new(UiMain::new(state)));
    builder.set_title("Synthesizer IO");
    let window = builder.build().unwrap();
    let _midi_connection = setup_midi(engine);  // keep from being dropped
    thread::spawn(move || run_cpal(worker));
    window.show();
    run_loop.run();
}

fn setup_midi(engine: Arc<Mutex<Engine>>) -> Option<MidiInputConnection<()>> {
    let mut midi_in = MidiInput::new("midir input").expect("can't create midi input");
    midi_in.ignore(::midir::Ignore::None);
    let result = midi_in.connect(0, "in", move |_ts, data, _| {
        //println!("{}, {:?}", ts, data);
        let mut engine = engine.lock().unwrap();
        engine.dispatch_midi(data, time::precise_time_ns());
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
