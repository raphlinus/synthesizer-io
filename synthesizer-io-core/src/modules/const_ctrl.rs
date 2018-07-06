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

//! A simple module that just sets a constant control parameter.

use module::{Module, Buffer};

pub struct ConstCtrl {
    value: f32,
}

impl ConstCtrl {
    pub fn new(value: f32) -> ConstCtrl {
        ConstCtrl { value: value }
    }
}

impl Module for ConstCtrl {
    fn n_ctrl_out(&self) -> usize { 1 }

    fn process(&mut self, _control_in: &[f32], control_out: &mut [f32],
        _buf_in: &[&Buffer], _buf_out: &mut [Buffer])
    {
        control_out[0] = self.value;
    }
}
