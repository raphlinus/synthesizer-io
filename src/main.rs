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

extern crate coreaudio;

#[macro_use]
extern crate lazy_static;

use coreaudio::audio_unit::{AudioUnit, IOType, SampleFormat};
use coreaudio::audio_unit::render_callback::{self, data};

mod queue;
mod graph;
mod module;
mod modules;
mod worker;

use worker::Worker;
use graph::Node;
use module::N_SAMPLES_PER_CHUNK;

fn main() {
    let (mut worker, tx, rx) = Worker::create(1024);
    let module = Box::new(modules::sin::Sin::new(440.0 / 44_100.0));
    let node = Node::create(module, 1, [], []);
    worker.handle_node(node);
    let module = Box::new(modules::sin::Sin::new(880.0 / 44_100.0));
    let node = Node::create(module, 2, [], []);
    worker.handle_node(node);
    let module = Box::new(modules::sum::Sum);
    let node = Node::create(module, 0, vec![(1, 0), (2, 0)], []);
    worker.handle_node(node);
    let _audio_unit = run(worker).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1_000));
    let module = Box::new(modules::sin::Sin::new(440.0 * 1.5 / 44_100.0));
    let node = Node::create(module, 2, [], []);
    tx.send(node);
    std::thread::sleep(std::time::Duration::from_millis(1_000));
}

fn run(mut worker: Worker) -> Result<AudioUnit, coreaudio::Error> {

    // Construct an Output audio unit that delivers audio to the default output device.
    let mut audio_unit = AudioUnit::new(IOType::DefaultOutput)?;

    let stream_format = audio_unit.stream_format()?;
    //println!("{:#?}", &stream_format);

    // We expect `f32` data.
    assert!(SampleFormat::F32 == stream_format.sample_format);

    type Args = render_callback::Args<data::NonInterleaved<f32>>;
    audio_unit.set_render_callback(move |args| {
        let Args { num_frames, mut data, .. } = args;
        assert!(num_frames % N_SAMPLES_PER_CHUNK == 0);
        let mut i = 0;
        while i < num_frames {
            // should let the graph generate stereo
            let buf = worker.work()[0].get();
            for j in 0..N_SAMPLES_PER_CHUNK {
                for channel in data.channels_mut() {
                    channel[i + j] = buf[j];
                }
            }
            i += N_SAMPLES_PER_CHUNK;
        }
        Ok(())
    })?;
    audio_unit.start()?;

    Ok(audio_unit)
}
