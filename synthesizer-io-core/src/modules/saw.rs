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

//! A module that makes a band-limited sawtooth wave.

use std::f32::consts;
use std::ops::Deref;
use std::cmp::min;

use module::{Module, Buffer};

const LG_N_SAMPLES: usize = 10;
const N_SAMPLES: usize = (1 << LG_N_SAMPLES);
const N_PARTIALS_MAX: usize = N_SAMPLES / 2;

const LG_SLICES_PER_OCTAVE: usize = 2;
const SLICES_PER_OCTAVE: usize = (1 << LG_SLICES_PER_OCTAVE);
const N_SLICES: usize = 36;
// 0.5 * (log(440./44100) / log(2) + log(440./48000) / log(2) + 2./12) + 1./64 - 3
const SLICE_BASE: f32 = -9.609300863499751;
const SLICE_OVERLAP: f32 = 0.125;

lazy_static! {
    static ref SAWTAB: [[f32; N_SAMPLES + 1]; N_SLICES] = {
        let mut t = [[0.0; N_SAMPLES + 1]; N_SLICES];

        let mut lut = [0.0; N_SAMPLES / 2];
        let slice_inc = (1.0 / SLICES_PER_OCTAVE as f32).exp2();
        let mut f_0 = slice_inc.powi(N_SLICES as i32 - 1) * SLICE_BASE.exp2();
        let mut n_partials_last = 0;
        for j in (0..N_SLICES).rev() {
            let n_partials = (0.5 / f_0) as usize;
            let n_partials = min(n_partials, N_PARTIALS_MAX);
            for k in n_partials_last + 1 .. n_partials + 1 {
                let mut scale = -consts::FRAC_2_PI / k as f32;
                if N_PARTIALS_MAX - k <= N_PARTIALS_MAX >> 2 {
                    scale *= (N_PARTIALS_MAX - k) as f32 * (1.0 / (N_PARTIALS_MAX >> 2) as f32);
                }
                let dphase = k as f64 * (2.0 * ::std::f64::consts::PI / N_SAMPLES as f64);
                let c = dphase.cos();
                let s = dphase.sin();
                let mut u = scale as f64;
                let mut v = 0.0f64;
                for i in 0..(N_SAMPLES / 2) {
                    lut[i] += v;
                    let t = u * s + v * c;
                    u = u * c - v * s;
                    v = t;
                }
            }
            for i in 1..(N_SAMPLES / 2) {
                let value = lut[i] as f32;
                t[j][i] = value;
                t[j][N_SAMPLES - i] = -value;
            }
            // note: values at 0, N_SAMPLES / 2 and N_SAMPLES all 0
            n_partials_last = n_partials;
            f_0 *= 1.0 / slice_inc;
        }
        t
    };
}

pub struct Saw {
    sr_offset: f32,
    phase: f32,
}

impl Saw {
    pub fn new(sample_rate: f32) -> Saw {
        // make initialization happen here so it doesn't happen in process
        let _ = SAWTAB.deref();
        Saw {
            sr_offset: LG_N_SAMPLES as f32 - sample_rate.log2(),
            phase: 0.0,
        }
    }
}

fn compute(tab_ix: usize, phasefrac: f32) -> f32 {
    (tab_ix as f32 + phasefrac) * (2.0 / N_SAMPLES as f32) - 1.0
}

impl Module for Saw {
    fn n_bufs_out(&self) -> usize { 1 }

    fn process(&mut self, control_in: &[f32], _control_out: &mut [f32],
        _buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let logf = control_in[0] + self.sr_offset;
        let slice_off = -SLICE_BASE - LG_N_SAMPLES as f32;
        let slice = (logf + slice_off) * SLICES_PER_OCTAVE as f32;
        //println!("logf={}, slice={}", logf, slice);
        let freq = logf.exp2();
        let out = buf_out[0].get_mut();
        let mut phase = self.phase;
        if slice < -SLICE_OVERLAP {
            // pure computation
            for i in 0..out.len() {
                let phaseint = phase as i32;
                let tab_ix = phaseint as usize % N_SAMPLES;
                let phasefrac = phase - phaseint as f32;
                out[i] = compute(tab_ix, phasefrac);
                phase += freq;
            }
        } else if slice < 0.0 {
            // interpolate between computation and slice 0
            let tab = &SAWTAB[0];
            let yi = slice * (-1.0 / SLICE_OVERLAP); // 1 = comp, 0 = lut
            for i in 0..out.len() {
                let phaseint = phase as i32;
                let tab_ix = phaseint as usize % N_SAMPLES;
                let phasefrac = phase - phaseint as f32;
                let yc = compute(tab_ix, phasefrac);
                let y0 = tab[tab_ix];
                let y1 = tab[tab_ix + 1];
                let yl = y0 + (y1 - y0) * phasefrac;
                out[i] = yl + yi * (yc - yl);
                phase += freq;
            }
        } else {
            let tab = SAWTAB.deref();
            let sliceint = slice as u32;
            let slicefrac = slice - sliceint as f32;
            if slicefrac < 1.0 - SLICE_OVERLAP || sliceint >= N_SLICES as u32 - 1 {
                // do lookup from a single slice
                let tab = &tab[min(sliceint as usize, N_SLICES - 1)];
                for i in 0..out.len() {
                    let phaseint = phase as i32;
                    let tab_ix = phaseint as usize % N_SAMPLES;
                    let y0 = tab[tab_ix];
                    let y1 = tab[tab_ix + 1];
                    out[i] = y0 + (y1 - y0) * (phase - phaseint as f32);
                    phase += freq;
                }
            } else {
                // interpolate between two slices
                let tab0 = &tab[sliceint as usize];
                let tab1 = &tab[1 + sliceint as usize];
                let yi = (slicefrac - (1.0 - SLICE_OVERLAP)) * (1.0 / SLICE_OVERLAP);
                for i in 0..out.len() {
                    let phaseint = phase as i32;
                    let tab_ix = phaseint as usize % N_SAMPLES;
                    let phasefrac = phase - phaseint as f32;
                    let y00 = tab0[tab_ix];
                    let y01 = tab0[tab_ix + 1];
                    let y0 = y00 + (y01 - y00) * phasefrac;
                    let y10 = tab1[tab_ix];
                    let y11 = tab1[tab_ix + 1];
                    let y1 = y10 + (y11 - y10) * phasefrac;
                    out[i] = y0 + yi * (y1 - y0);
                    phase += freq;
                }
            }
        }
        let phaseint = phase as i32;
        self.phase = phase - (phaseint & -(N_SAMPLES as i32)) as f32;
    }
}
