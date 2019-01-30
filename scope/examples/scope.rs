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

//! A testbed for experimenting with scope display.

extern crate png;

extern crate synthesize_scope;

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use png::HasParameters;

use synthesize_scope::gauss_approx;
use synthesize_scope::Scope;

fn mk_uvmap_img<F>(f: F) -> Vec<u8>
where
    F: Fn(f32, f32) -> f32,
{
    let w = 640;
    let h = 480;
    let scale = 2.0 / (w.min(h) as f32);
    let xs = scale;
    let x0 = -0.5 * (w as f32) * scale;
    let ys = scale;
    let y0 = -0.5 * (h as f32) * scale;

    let mut im = vec![255; w * h * 4];
    for y in 0..h {
        let v = (y as f32) * ys + y0;
        for x in 0..w {
            let u = (x as f32) * xs + x0;
            let z = f(u, v);
            let g = (255.0 * z.max(0.0).min(1.0)) as u8;
            let ix = (y * w + x) * 4;
            im[ix + 0] = g;
            im[ix + 1] = g;
            im[ix + 2] = g;
        }
    }
    im
}

fn main() {
    let path = Path::new("foo.png");
    let f = File::create(path).unwrap();
    let w = BufWriter::new(f);
    let mut encoder = png::Encoder::new(w, 640, 480);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    /*
    let z = 2.0 / ::std::f32::consts::PI.sqrt();
    let img = mk_uvmap_img(|u, v|
        gauss_approx(v * 5.0) * 0.5 * (erf_approx(u * 5.0 * z) - erf_approx((u - 0.5) * 5.0 * z))
    );
    */
    let mut scope = Scope::new(640, 480);
    let r = 1.0;
    let start = ::std::time::Instant::now();
    let mut xylast = None;
    // sinewave!
    for i in 0..1001 {
        let h = (i as f32) * 0.001;
        let x = 640.0 * h;
        let y = 240.0 + 200.0 * (h * 50.0).sin();
        if let Some((xlast, ylast)) = xylast {
            scope.add_line(xlast, ylast, x, y, r, 2.0);
        }
        xylast = Some((x, y));
    }
    println!("elapsed: {:?}", start.elapsed());
    let img = scope.as_rgba();
    println!("elapsed after rgba: {:?}", start.elapsed());
    writer.write_image_data(&img).unwrap();
}
