// Copyright 2018 The Synthesizer IO Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Benchmarks of sigmoid functions.

#![feature(test)]

extern crate test;

use std::arch::x86_64::*;

fn compute_std_alg(inp: &[f32], out: &mut [f32]) {
    for (x, y) in inp.iter().zip(out.iter_mut()) {
        *y = x / (1.0 + x * x).sqrt();
    }
}

// max error 2e-4
fn compute_tanh5(inp: &[f32], out: &mut [f32]) {
    for (x, y) in inp.iter().zip(out.iter_mut()) {
        let xx = x * x;
        let x = x + (0.16489087 + 0.00985468 * xx) * (x * xx);
        *y = x / (1.0 + x * x).sqrt();
    }
}

// Note: this is scaled for a slope of 1 at the origin, ie it computes
// erf(x * sqrt(pi) / 2).

fn compute_erf7(inp: &[f32], out: &mut [f32]) {
    for (x, y) in inp.iter().zip(out.iter_mut()) {
        let xx = x * x;
        let x = x + (0.24295 + (0.03395 + 0.0104 * xx) * xx) * (x * xx);
        *y = x / (1.0 + x * x).sqrt();
    }
}

// max error ~1.5e-3
// from https://arxiv.org/pdf/1702.07825.pdf
fn compute_tanh_etilde(inp: &[f32], out: &mut [f32]) {
    for (x, y) in inp.iter().zip(out.iter_mut()) {
        let xx = x * x;
        let etilde = 1.0 + x.abs() + (0.5658 + 0.143 * xx) * xx;
        let erecip = etilde.recip();
        *y = x.signum() * (etilde - erecip) / (etilde + erecip);
    }
}

// Approximation from Abramowitz and Stegun (max error 5e-4)
fn compute_erf_as(inp: &[f32], out: &mut [f32]) {
    for (x, y) in inp.iter().zip(out.iter_mut()) {
        let a = x.abs();
        let b = 1.0 + (0.278393 + (0.230389 + (0.000972 + 0.078108 * a) * a) * a) * a;
        let b2 = b * b;
        let b4 = b2 * b2;
        *y = x.signum() * (1.0 - b4.recip());
    }
}

fn compute_simd_alg(inp: &[f32], out: &mut [f32]) {
    let n = inp.len() / 4;
    unsafe {
        for i in 0..n {
            let x = _mm_loadu_ps(inp.as_ptr().offset(i as isize * 4));
            let r = _mm_add_ps(_mm_set1_ps(1.0), _mm_mul_ps(x, x));
            let est = _mm_rsqrt_ps(r);
            let r_est = _mm_mul_ps(r, est);
            let half_est = _mm_mul_ps(_mm_set1_ps(0.5), est);
            let muls = _mm_mul_ps(r_est, est);
            let three_minus_muls = _mm_sub_ps(_mm_set1_ps(3.0), muls);
            let refined = _mm_mul_ps(half_est, three_minus_muls);
            let y = _mm_mul_ps(x, refined);
            _mm_storeu_ps(out.as_mut_ptr().offset(i as isize * 4), y);
        }
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use test::Bencher;

    // Number of functions evaluated in single loop
    const N: usize = 128;

    #[bench]
    fn std_tanh(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| {
            for (x, y) in inp.iter().zip(out.iter_mut()) {
                *y = x.tanh()
            }
        })
    }

    #[bench]
    fn std_alg(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| compute_std_alg(&inp, &mut out));
    }

    #[bench]
    fn tanh5(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| compute_tanh5(&inp, &mut out));
    }

    #[bench]
    fn erf7(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| compute_erf7(&inp, &mut out));
    }

    #[bench]
    fn tanh_etilde(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| compute_tanh_etilde(&inp, &mut out));
    }

    #[bench]
    fn erf_as(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| compute_erf_as(&inp, &mut out));
    }

    #[bench]
    fn simd_alg(b: &mut Bencher) {
        let inp = [0.1f32; N];
        let mut out = [0.0f32; N];
        b.iter(|| compute_simd_alg(&inp, &mut out));
    }
}
