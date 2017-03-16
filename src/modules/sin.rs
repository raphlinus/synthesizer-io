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

use std::f32::consts;
use std::ops::Deref;

use module::{Module, Buffer};

const LG_N_SAMPLES: usize = 10;
const N_SAMPLES: usize = (1 << LG_N_SAMPLES);

lazy_static! {
    static ref SINTAB: [f32; N_SAMPLES + 1] = {
        let mut t = [0.0; N_SAMPLES + 1];
        let dth = 2.0 * consts::PI / (N_SAMPLES as f32);
        for i in 0..N_SAMPLES/2 {
            let s = (i as f32 * dth).sin();
            t[i] = s;
            t[i + N_SAMPLES / 2] = -s;
        }
        // TODO: more optimization is possible
        // t[N_SAMPLES] = t[0], but not necessary because it's already 0
        t
    };
}

pub struct Sin {
    sr_offset: f32,
    phase: f32,
}

impl Sin {
    pub fn new(sample_rate: f32) -> Sin {
        // make initialization happen here so it doesn't happen in process
        let _ = SINTAB.deref();
        Sin {
            sr_offset: LG_N_SAMPLES as f32 - sample_rate.log2(),
            phase: 0.0,
        }
    }
}

impl Module for Sin {
    fn n_bufs_out(&self) -> usize { 1 }

    // Example of migration, although replacing one Sin module with another
    // isn't going to have much use unless the sample rate is changing. But
    // if so, at least the phase will be continuous now.
    fn migrate(&mut self, old: &mut Module) {
        if let Some(old_sin) = old.to_any().downcast_ref::<Sin>() {
            self.phase = old_sin.phase;
        }
    }

    fn process(&mut self, control_in: &[f32], _control_out: &mut [f32],
        _buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let freq = (control_in[0] + self.sr_offset).exp2();
        let tab = SINTAB.deref();
        let out = buf_out[0].get_mut();
        let mut phase = self.phase;
        for i in 0..out.len() {
            let phaseint = phase as i32;
            let tab_ix = phaseint as usize % N_SAMPLES;
            let y0 = tab[tab_ix];
            let y1 = tab[tab_ix + 1];
            out[i] = y0 + (y1 - y0) * (phase - phaseint as f32);
            phase += freq;
        }
        let phaseint = phase as i32;
        self.phase = phase - (phaseint & -(N_SAMPLES as i32)) as f32;
    }
}
