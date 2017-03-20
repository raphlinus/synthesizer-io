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

//! Attack, decay, sustain, release.

use module::{Module, Buffer};

pub struct Adsr {
    value: f32,
    state: State,
}

enum State {
    Quiet,
    Attack,  // note is on, rising
    Decay,  // note is on, falling
    Sustain,  // note is on, steady
    Release,  // note is off, falling
}

use self::State::*;

impl Adsr {
    pub fn new() -> Adsr {
        Adsr {
            value: -24.0,
            state: Quiet,
        }
    }
}

impl Module for Adsr {
    fn n_ctrl_out(&self) -> usize { 1 }

    fn handle_note(&mut self, _midi_num: f32, _velocity: f32, on: bool) {
        if on {
            self.state = Attack;
        } else {
            self.state = Release;
        }
    }

    fn process(&mut self, control_in: &[f32], control_out: &mut [f32],
        _buf_in: &[&Buffer], _buf_out: &mut [Buffer])
    {
        match self.state {
            Quiet => (),
            Attack => {
                let mut l = self.value.exp2();
                l += (-control_in[0]).exp2();
                if l >= 1.0 {
                    l = 1.0;
                    self.state = Decay;
                }
                self.value = l.log2();
            }
            Decay => {
                let sustain = control_in[2] - 6.0;
                self.value -= (-control_in[1]).exp2();
                if self.value < sustain {
                    self.value = sustain;
                    self.state = Sustain;
                }
            }
            Sustain => {
                let sustain = control_in[2] - 6.0;
                self.value = sustain;
            }
            Release => {
                self.value -= (-control_in[3]).exp2();
                if self.value < -24.0 {
                    self.value = -24.0;
                    self.state = Quiet;
                }
            }
        }
        control_out[0] = self.value;
    }
}
