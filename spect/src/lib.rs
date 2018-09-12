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

//! A spectrum analyzer.

extern crate rustfft;
mod colormap;

use std::sync::Arc;
use std::f32::consts::PI;

use rustfft::{FFT, FFTplanner};
use rustfft::num_complex::Complex;

pub struct Spect {
    window: Vec<f32>,
    ibuf: Vec<Complex<f32>>,
    obuf: Vec<Complex<f32>>,
    fft: Arc<FFT<f32>>,
}

impl Spect {
    pub fn new(width: usize) -> Spect {
        let mut planner = FFTplanner::new(false);
        let fft = planner.plan_fft(width);
        let window = Self::mk_window(width);
        let ibuf = vec![Default::default(); width];
        let obuf = vec![Default::default(); width];
        Spect { window, ibuf, obuf, fft }
    }

    pub fn image_dims(&self, n_samples: usize) -> (usize, usize) {
        let height = self.window.len() / 2;
        let width = n_samples / height - 1;
        (width, height)
    }

    /// Generates RGBA pixels (suitable for use by the PNG crate).
    pub fn generate(&mut self, input: &[f32]) -> Vec<u8> {
        let (width, height) = self.image_dims(input.len());
        let mut img = vec![255; 4 * width * height];
        let window_len = self.window.len();
        let step = window_len / 2;
        let mut ix = 0;
        for x in 0..width {
            self.compute_one_window(&input[ix..ix + window_len]);
            self.fill_column(&mut img, x, width);
            ix += step;
        }
        img
    }

    // Compute one slice worth of spectrum. On input, `data` is the same size as the window.
    fn compute_one_window(&mut self, data: &[f32]) {
        for ((i, w), o) in data.iter().zip(self.window.iter()).zip(self.ibuf.iter_mut()) {
            *o = (i * w).into();
        }
        self.fft.process(&mut self.ibuf, &mut self.obuf);
    }

    fn fill_column(&self, img: &mut [u8], x: usize, width: usize) {
        // TODO: make scaling parameters tunable in constructor
        let max_amp = 40.0;  // dB
        let min_amp = max_amp - 120.0;

        let y_scale = 255.0 * 10.0 / 10f32.ln() / (max_amp - min_amp);
        let y0 = 255.0 - y_scale * max_amp * 10f32.ln() / 10.0;
        let height = self.window.len() / 2;
        let stride = width * 4;
        let mut ix = x * 4 + height * stride;
        for z in &self.obuf[0..height] {
            ix -= stride;
            let y = (z.norm_sqr() + 1e-12).ln();
            let scaled_y = y0 + y * y_scale;
            //println!("z = {:?}, y {}, sc_y = {}", z, y, scaled_y);
            let (r, g, b) = colormap::map_inferno(scaled_y);
            img[ix] = r;
            img[ix + 1] = g;
            img[ix + 2] = b;
        }
    }

    // Create a Hann window of the specified width.
    fn mk_window(width: usize) -> Vec<f32> {
        let d = 2.0 * PI / (width as f32);
        (0..width).map(|i| 0.5 - 0.5 * (i as f32 * d).cos()).collect()
    }
}
