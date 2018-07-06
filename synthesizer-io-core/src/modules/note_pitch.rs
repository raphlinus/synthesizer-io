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

//! A simple module that just holds a note at a constant pitch.

use module::{Module, Buffer};

pub struct NotePitch {
    value: f32,
}

impl NotePitch {
    pub fn new() -> NotePitch {
        NotePitch { value: 0.0 }
    }
}

impl Module for NotePitch {
    fn n_ctrl_out(&self) -> usize { 1 }

    fn handle_note(&mut self, midi_num: f32, _velocity: f32, on: bool) {
        if on {
            self.value = midi_num * (1.0 / 12.0) + (440f32.log2() - 69.0 / 12.0);
        }
    }

    fn process(&mut self, _control_in: &[f32], control_out: &mut [f32],
        _buf_in: &[&Buffer], _buf_out: &mut [Buffer])
    {
        control_out[0] = self.value;
    }
}
