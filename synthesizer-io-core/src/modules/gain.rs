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

//! A simple module that applies gain to the input. Gain is interpreted
//! as log2 of absolute gain. Linear smoothing applied.

use module::{Module, Buffer};

pub struct Gain {
    last_g: f32,
}

impl Gain {
    pub fn new() -> Gain {
        Gain {
            last_g: 0.0,
        }
    }
}

impl Module for Gain {
    fn n_bufs_out(&self) -> usize { 1 }

    fn process(&mut self, control_in: &[f32], _control_out: &mut [f32],
        buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let ctrl = control_in[0];
        let g = ctrl.exp2();
        let out = buf_out[0].get_mut();
        let dg = (g - self.last_g) * (1.0 / out.len() as f32);
        let mut y = self.last_g + dg;
        self.last_g = g;
        let buf = buf_in[0].get();
        for i in 0..out.len() {
            out[i] = buf[i] * y;
            y += dg;
        }
    }
}
