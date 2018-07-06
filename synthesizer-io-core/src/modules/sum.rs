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

//! A simple module that just sums the inputs.

use module::{Module, Buffer};

pub struct Sum;

impl Module for Sum {
    fn n_bufs_out(&self) -> usize { 1 }

    fn process(&mut self, _control_in: &[f32], _control_out: &mut [f32],
        buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let out = buf_out[0].get_mut();
        for i in 0..out.len() {
            out[i] = 0.0;
        }
        for buf in buf_in {
            let buf = buf.get();
            for i in 0..out.len() {
                out[i] += buf[i];
            }
        }
    }
}
