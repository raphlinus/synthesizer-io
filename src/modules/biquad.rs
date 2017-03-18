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

//! An implementation of biquad filters.


use std::f32::consts;

use module::{Module, Buffer};

pub struct Biquad {
    sr_offset: f32,
    state: [f32; 2],
    matrix: [f32; 16],
}

impl Biquad {
    pub fn new(sample_rate: f32) -> Biquad {
        Biquad {
            sr_offset: consts::PI.log2() - sample_rate.log2(),
            state: [0.0; 2],
            matrix: [0.0; 16],
        }
    }
}

struct StateParams {
    a: [f32; 4],  // 2x2 matrix, column-major order
    b: [f32; 2],
    c: [f32; 2],
    d: f32,
}

// `log_f` is log2 of frequency relative to sampling rate, e.g.
// -1.0 is the Nyquist frequency.
fn calc_g(log_f: f32) -> f32 {
    // TODO: use lut to speed this up
    let f = log_f.exp2();  // pi has already been factored into sr_offset
    f.tan()
}

// Compute parameters for low-pass state variable filter.
// `res` ranges from 0 (no resonance) to 1 (self-oscillating)
fn svf_lp(log_f: f32, res: f32) -> StateParams {
    let g = calc_g(log_f);
    let k = 2.0 - 2.0 * res;
    let a1 = 2.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    let a = [a1 - 1.0, a2, -a2, 1.0 - a3];
    let b = [a2, a3];
    let c = [0.5 * a2, 1.0 - 0.5 * a3];
    let d = 0.5 * a3;
    StateParams { a: a, b: b, c: c, d: d }
}

// See https://github.com/google/music-synthesizer-for-android/blob/master/lab/Second%20order%20sections%20in%20matrix%20form.ipynb
fn raise_matrix(params: StateParams) -> [f32; 16] {
    let StateParams { a, b, c, d } = params;
    [d, c[0] * b[0] + c[1] * b[1],
     a[0] * b[0] + a[2] * b[1], a[1] * b[0] + a[3] * b[1],

     0.0, d, b[0], b[1],

     c[0], c[0] * a[0] + c[1] * a[1],
     a[0] * a[0] + a[2] * a[1], a[1] * a[0] + a[3] * a[1],

     c[1], c[0] * a[2] + c[1] * a[3],
     a[0] * a[2] + a[2] * a[3], a[1] * a[2] + a[3] * a[3],
    ]
}

impl Module for Biquad {
    fn n_bufs_out(&self) -> usize { 1 }

    fn process(&mut self, control_in: &[f32], _control_out: &mut [f32],
        buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let log_f = control_in[0];
        let res = control_in[1];
        // TODO: maybe avoid recomputing matrix if params haven't changed
        let params = svf_lp(log_f + self.sr_offset, res);
        self.matrix = raise_matrix(params);
        let inb = buf_in[0].get();
        let out = buf_out[0].get_mut();
        let m = &self.matrix;
        let mut i = 0;
        let mut state0 = self.state[0];
        let mut state1 = self.state[1];
        while i < out.len() {
            let x0 = inb[i];
            let x1 = inb[i + 1];
            let y0 = m[0] * x0 + m[4] * x1 + m[8] * state0 + m[12] * state1;
            let y1 = m[1] * x0 + m[5] * x1 + m[9] * state0 + m[13] * state1;
            let y2 = m[2] * x0 + m[6] * x1 + m[10] * state0 + m[14] * state1;
            let y3 = m[3] * x0 + m[7] * x1 + m[11] * state0 + m[15] * state1;
            out[i] = y0;
            out[i + 1] = y1;
            state0 = y2;
            state1 = y3;
            i += 2;
        }
        self.state[0] = state0;
        self.state[1] = state1;
    }
}
