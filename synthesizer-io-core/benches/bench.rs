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
extern crate synthesizer_io_core;

#[cfg(test)]
mod bench {
    use test::Bencher;
    use synthesizer_io_core::module::{Module, Buffer};
    use synthesizer_io_core::modules::Sin;
    use synthesizer_io_core::modules::Biquad;

    #[bench]
    fn sin(b: &mut Bencher) {
        let mut buf = [Buffer::default(); 1];
        let freq = [440.0f32.log2()];
        let mut sin = Sin::new(44_100.0);
        b.iter(||
            sin.process(&freq[..], &mut[][..], &[][..], &mut buf[..])
        )
    }

    #[bench]
    fn biquad(b: &mut Bencher) {
        let buf = Buffer::default();
        let bufs = [&buf];
        let mut bufo = [Buffer::default(); 1];
        let mut biquad = Biquad::new(44_100.0);
        let params = [44.0f32.log2(), 0.293];
        b.iter(||
            biquad.process(&params[..], &mut [][..], &bufs[..], &mut bufo[..])
        )
    }

    #[bench]
    fn direct_biquad(b: &mut Bencher) {
        // biquad in transposed direct form II architecture
        let a0 = 0.2513790015131591f32;
        let a1 = 0.5027580030263182f32;
        let a2 = 0.2513790015131591f32;
        let b1 = -0.17124071441396285f32;
        let b2 = 0.1767567204665992f32;
        let buf = Buffer::default();
        let mut bufo = Buffer::default();
        let inb = buf.get();
        let outb = bufo.get_mut();
        let mut z1 = 0.0f32;
        let mut z2 = 0.0f32;
        b.iter(||
            for i in 0..outb.len() {
                let inp = inb[i];
                let out = inp * a0 + z1;
                z1 = inp * a1 + z2 - b1 * out;
                z2 = inp * a2 - b2 * out;
                outb[i] = out;
            }
        )
    }

    #[bench]
    fn exp2(b: &mut Bencher) {
        b.iter(|| {
            let mut y = 0.0;
            for i in 0..1000 {
                y += (0.001 * i as f32).exp2();
            }
            y
        })
    }

    #[bench]
    fn tan(b: &mut Bencher) {
        b.iter(|| {
            let mut y = 0.0;
            for i in 0..1000 {
                y += (0.001 * i as f32).tan();
            }
            y
        })
    }
}
