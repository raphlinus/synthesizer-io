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

//! Benchmarks for various audio processing things.

#![feature(test)]

extern crate test;
extern crate synthesizer_io;

#[cfg(test)]
mod bench {
    use test::Bencher;
    use synthesizer_io::module::{Module, Buffer};
    use synthesizer_io::modules::sin::Sin;

    #[bench]
    fn sin(b: &mut Bencher) {
        let mut buf = [Buffer::default(); 1];
        let mut sin = Sin::new(440.0 / 44_100.0);
        b.iter(||
            sin.process(&[][..], &mut[][..], &[][..], &mut buf[..])
        )
    }
}
