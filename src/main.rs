// Copyright 2017 The Synthesizer IO Authors.
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

#[cfg(target_os = "macos")]
extern crate coreaudio;
#[cfg(target_os = "macos")]
extern crate coremidi;

#[cfg(not(target_os = "macos"))]
extern crate cpal;
#[cfg(not(target_os = "macos"))]
extern crate midir;

extern crate time;

extern crate synthesizer_io_core;

#[cfg(target_os = "macos")]
use coreaudio::audio_unit::render_callback::{self, data};
#[cfg(target_os = "macos")]
use coreaudio::audio_unit::{AudioUnit, IOType, SampleFormat, Scope};

#[cfg(not(target_os = "macos"))]
use cpal::{EventLoop, StreamData, UnknownTypeOutputBuffer};

#[cfg(not(target_os = "macos"))]
use midir::MidiInput;
#[cfg(not(target_os = "macos"))]
use std::ops::DerefMut;

use synthesizer_io_core::modules;

use synthesizer_io_core::graph::{Message, Node, Note, SetParam};
use synthesizer_io_core::module::N_SAMPLES_PER_CHUNK;
use synthesizer_io_core::queue::Sender;
use synthesizer_io_core::worker::Worker;

struct Midi {
    tx: Sender<Message>,
    cur_note: Option<u8>,
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
        let value = lo + value as f32 * (1.0 / 127.0) * (hi - lo);
        let param = SetParam {
            ix: ix,
            param_ix: 0,
            val: value,
            timestamp: ts,
        };
        self.send(Message::SetParam(param));
    }

    fn send_note(&mut self, ixs: Vec<usize>, midi_num: f32, velocity: f32, on: bool, ts: u64) {
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
}

fn main() {
    let (mut worker, tx, _rx) = Worker::create(1024);

    /*
    let module = Box::new(modules::ConstCtrl::new(440.0f32.log2()));
    worker.handle_node(Node::create(module, 1, [], []));
    let module = Box::new(modules::Sin::new(44_100.0));
    worker.handle_node(Node::create(module, 2, [], [(1, 0)]));
    let module = Box::new(modules::ConstCtrl::new(880.0f32.log2()));
    worker.handle_node(Node::create(module, 3, [], []));
    let module = Box::new(modules::Sin::new(44_100.0));
    worker.handle_node(Node::create(module, 4, [], [(3, 0)]));
    let module = Box::new(modules::Sum);
    worker.handle_node(Node::create(module, 0, [(2, 0), (4, 0)], []));
    */

    let module = Box::new(modules::Saw::new(44_100.0));
    worker.handle_node(Node::create(module, 1, [], [(5, 0)]));
    let module = Box::new(modules::SmoothCtrl::new(880.0f32.log2()));
    worker.handle_node(Node::create(module, 3, [], []));
    let module = Box::new(modules::SmoothCtrl::new(0.5));
    worker.handle_node(Node::create(module, 4, [], []));
    let module = Box::new(modules::NotePitch::new());
    worker.handle_node(Node::create(module, 5, [], []));
    let module = Box::new(modules::Biquad::new(44_100.0));
    worker.handle_node(Node::create(module, 6, [(1, 0)], [(3, 0), (4, 0)]));
    let module = Box::new(modules::Adsr::new());
    worker.handle_node(Node::create(
        module,
        7,
        [],
        vec![(11, 0), (12, 0), (13, 0), (14, 0)],
    ));
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

    #[cfg(target_os = "macos")]
    run_mac(worker, tx);

    #[cfg(not(target_os = "macos"))]
    run_cpal(worker, tx);
}

#[cfg(not(target_os = "macos"))]
fn run_cpal(mut worker: Worker, tx: Sender<Message>) {
    let event_loop = EventLoop::new();
    let device = cpal::default_output_device().expect("no output device");
    let mut supported_formats_range = device
        .supported_output_formats()
        .expect("error while querying formats");
    let format = supported_formats_range
        .next()
        .expect("no supported format?!")
        .with_max_sample_rate();
    println!("format: {:?}", format);
    let stream_id = event_loop.build_output_stream(&device, &format).unwrap();
    event_loop.play_stream(stream_id);

    // midi setup
    let mut midi = Midi::new(tx);

    let mut midi_in = MidiInput::new("midir input").expect("can't create midi input");
    midi_in.ignore(::midir::Ignore::None);
    let result = midi_in.connect(
        0,
        "in",
        move |ts, data, _| {
            //println!("{}, {:?}", ts, data);
            midi.dispatch_midi(data, ts);
        },
        (),
    );
    if let Err(e) = result {
        println!("error connecting to midi: {:?}", e);
    }

    event_loop.run(move |_stream_id, stream_data| {
        match stream_data {
            StreamData::Output {
                buffer: UnknownTypeOutputBuffer::F32(mut buf),
            } => {
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

#[cfg(target_os = "macos")]
fn run_mac(worker: Worker, tx: Sender<Message>) {
    let _audio_unit = run_audio_unit(worker).unwrap();

    let source_index = 0;
    if let Some(source) = coremidi::Source::from_index(source_index) {
        println!("Listening for midi from {}", source.display_name().unwrap());
        let client = coremidi::Client::new("synthesizer-client").unwrap();
        let mut last_ts = 0;
        let mut last_val = 0;
        let mut midi = Midi::new(tx);
        let callback = move |packet_list: &coremidi::PacketList| {
            for packet in packet_list.iter() {
                let data = packet.data();
                let delta_t = packet.timestamp() - last_ts;
                let speed = 1e9 * (data[2] as f64 - last_val as f64) / delta_t as f64;
                println!(
                    "{} {:3.3} {} {}",
                    speed,
                    delta_t as f64 * 1e-6,
                    data[2],
                    time::precise_time_ns() - packet.timestamp()
                );
                last_val = data[2];
                last_ts = packet.timestamp();
                midi.dispatch_midi(&data, last_ts);
            }
        };
        let input_port = client.input_port("synthesizer-port", callback).unwrap();
        input_port.connect_source(&source).unwrap();

        println!("Press Enter to exit.");
        let mut line = String::new();
        ::std::io::stdin().read_line(&mut line).unwrap();
        input_port.disconnect_source(&source).unwrap();
    } else {
        println!("No midi available");
    }
}

#[cfg(target_os = "macos")]
fn run_audio_unit(mut worker: Worker) -> Result<AudioUnit, coreaudio::Error> {
    // Construct an Output audio unit that delivers audio to the default output device.
    let mut audio_unit = AudioUnit::new(IOType::DefaultOutput)?;

    let stream_format = audio_unit.stream_format(Scope::Output)?;
    //println!("{:#?}", &stream_format);

    // We expect `f32` data.
    assert!(SampleFormat::F32 == stream_format.sample_format);

    type Args = render_callback::Args<data::NonInterleaved<f32>>;
    audio_unit.set_render_callback(move |args| {
        let Args {
            num_frames,
            mut data,
            ..
        }: Args = args;
        assert!(num_frames % N_SAMPLES_PER_CHUNK == 0);
        let mut i = 0;
        let mut timestamp = time::precise_time_ns();
        while i < num_frames {
            // should let the graph generate stereo
            let buf = worker.work(timestamp)[0].get();
            for j in 0..N_SAMPLES_PER_CHUNK {
                for channel in data.channels_mut() {
                    channel[i + j] = buf[j];
                }
            }
            // TODO: calculate properly, magic value is 64 * 1e9 / 44_100
            timestamp += 1451247 * (N_SAMPLES_PER_CHUNK as u64) / 64;
            i += N_SAMPLES_PER_CHUNK;
        }
        Ok(())
    })?;
    audio_unit.start()?;

    Ok(audio_unit)
}
