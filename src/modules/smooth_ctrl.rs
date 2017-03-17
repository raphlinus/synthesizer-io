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

//! A module that smooths parameters (optimized for midi controllers).

use module::{Module, Buffer};

pub struct SmoothCtrl {
    rate: f32,  // smoothed rate (units of updates per ms)
    rategoal: f32,  // unsmoothed rate
    t: u64,  // timestamp of current time
    last_set_t: u64,  // timestamp of last param setting
    inp: f32,  // raw, unsmoothed value
    mid: f32,  // result of 1 pole of lowpass filtering
    out: f32,  // result of 2 poles of lowpass filtering
}

impl SmoothCtrl {
    pub fn new(value: f32) -> SmoothCtrl {
        SmoothCtrl {
            rate: 0.0,
            rategoal: 0.0,
            t: 0,
            last_set_t: 0,
            inp: value,
            mid: value,
            out: value,
        }
    }
}

impl Module for SmoothCtrl {
    fn n_ctrl_out(&self) -> usize { 1 }

    // maybe empty impl belongs in Module?
    fn process(&mut self, _control_in: &[f32], _control_out: &mut [f32],
        _buf_in: &[&Buffer], _buf_out: &mut [Buffer])
    {
    }

    fn process_ts(&mut self, _control_in: &[f32], control_out: &mut [f32],
        _buf_in: &[&Buffer], _buf_out: &mut [Buffer], timestamp: u64)
    {
        self.advance_to(timestamp);
        control_out[0] = self.out;
    }

    fn set_param(&mut self, _param_ix: usize, val: f32, timestamp: u64) {
        self.advance_to(timestamp);
        if timestamp > self.last_set_t {
            const SLOWEST_RATE: f32 = 0.005;  // 0.2s
            let mut rategoal = 1e6 / ((timestamp - self.last_set_t) as f32);
            if rategoal <= SLOWEST_RATE {
                rategoal = SLOWEST_RATE;
            }
            self.rategoal = rategoal;
            self.last_set_t = timestamp;
        }
        self.inp = val;
    }
}

impl SmoothCtrl {
    // Analytic solutions of the 3 1-pole lowpass filters under step invariant assumption.
    fn advance_to(&mut self, t: u64) {
        if t <= self.t {
            return;
        }
        let dt = (t - self.t) as f32 * 1e-6;  // in ms
        const RATE_TC: f32 = 10.0;  // in ms
        let erate = ((-1.0 / RATE_TC) * dt).exp();
        let warped_dt = dt * self.rategoal + RATE_TC * (self.rate - self.rategoal) * (1.0 - erate);
        self.rate = self.rategoal + (self.rate - self.rategoal) * erate;
        let ewarp = (-warped_dt).exp();
        self.out = self.inp + (self.out - self.inp + (self.mid - self.inp) * warped_dt) * ewarp;
        self.mid = self.inp + (self.mid - self.inp) * ewarp;
        self.t = t;
    }
}
