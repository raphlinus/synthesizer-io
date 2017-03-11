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

//! A simple module that makes a sine wave.

use std::f64::consts;
use std::ops::Deref;

use module::{Module, Buffer};

const LG_N_SAMPLES: usize = 10;
const N_SAMPLES: usize = (1 << LG_N_SAMPLES);

lazy_static! {
    static ref SINTAB: [f32; N_SAMPLES + 1] = {
        let mut t = [0.0; N_SAMPLES + 1];
        let dth = 2.0 * consts::PI / (N_SAMPLES as f64);
        for i in 0..N_SAMPLES {
            t[i] = (i as f64 * dth).sin() as f32
        }
        // t[N_SAMPLES] = t[0], but not necessary because it's already 0
        t
    };
}

pub struct Sin {
    phase: f32,
    freq: f32,  // pre-scaled by N_SAMPLES
}

impl Sin {
    /// Frequency is specified in cycles per sample. Note: we'll move to freq as
    /// a control input.
    pub fn new(freq: f32) -> Sin {
        // make initialization happen here so it doesn't happen in process
        let _ = SINTAB.deref();
        Sin {
            phase: 0.0,
            freq: freq * N_SAMPLES as f32,
        }
    }
}

// Note: this generates poor code on rustc 1.14 with the generic x86_64 cpu target,
// but good code when target_cpu is set to penryn or later. There are techniques
// for better code; probably the best thing is to file a bug with Rust upstream.
fn mod_1(x: f32) -> f32 {
    x - x.floor()
}

impl Module for Sin {
    fn n_bufs_out(&self) -> usize { 1 }

    fn process(&mut self, _control_in: &[f32], _control_out: &mut [f32],
        _buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let tab = SINTAB.deref();
        let out = buf_out[0].get_mut();
        let mut phase = self.phase * N_SAMPLES as f32;
        for i in 0..out.len() {
            let tab_ix = phase as u32 as usize % N_SAMPLES;
            let y0 = tab[tab_ix];
            let y1 = tab[tab_ix + 1];
            out[i] = y0 + (y1 - y0) * mod_1(phase);
            phase += self.freq;
        }
        self.phase = mod_1(phase * (1.0 / N_SAMPLES as f32));
    }
}
