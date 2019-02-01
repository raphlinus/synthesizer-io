// Copyright 2018 The Synthesizer IO Authors.
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

//! A little sketch to make spectrograms of sigmoid functions applied to sines.

extern crate hound;
extern crate png;
extern crate synthesizer_io_spect;

use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use png::HasParameters;

use synthesizer_io_spect::Spect;

/// Approximate erf(x * sqrt(pi) / 2)
#[allow(unused)]
fn erf7(x: f32) -> f32 {
    let xx = x * x;
    let x = x + (0.24295 + (0.03395/*+ 0.0104 * xx*/) * xx) * (x * xx);
    x / (1.0 + x * x).sqrt()
}

fn gen_audio(len: usize) -> Vec<f32> {
    //(0..len).map(|i| ((i as f32).powi(2) * 1e-7).sin()).collect()
    let f = 440.0;
    let d = f / 44_100.0 * 2.0 * std::f64::consts::PI;
    (0..len)
        .map(|i| {
            let i = i as f64;
            let amp = 100.0 * (i * -4e-5).exp();
            let tone = (i * d).sin() * amp;
            tone.max(-1.0).min(1.0) as f32
            //erf7(tone as f32)
            //(tone / (1.0 + tone * tone).sqrt()) as f32
            //tone.tanh() as f32
        })
        .collect()
}

fn main() {
    let file_base = env::args().skip(1).next().expect("need filename base");

    let audio = gen_audio(441_000);

    // Write audio file as WAV
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let wav_path = Path::new(&file_base).with_extension("wav");
    let mut wav_writer = hound::WavWriter::create(wav_path, spec).unwrap();
    for y in &audio {
        let scale = 20000.0;
        let y_int = (scale * y).max(-32767.0).min(32767.0) as i16;
        wav_writer.write_sample(y_int).unwrap();
    }

    let mut spect = Spect::new(1024);
    let (width, height) = spect.image_dims(audio.len());
    let img = spect.generate(&audio);

    // Write spectrogram image as PNG
    let path = Path::new(&file_base).with_extension("png");
    let f = File::create(path).unwrap();
    let w = BufWriter::new(f);

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&img).unwrap();
}
