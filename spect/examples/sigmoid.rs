extern crate png;
extern crate synthesizer_io_spect;

use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
// To use encoder.set()
use png::HasParameters;

use synthesizer_io_spect::Spect;

fn gen_audio(len: usize) -> Vec<f32> {
    //(0..len).map(|i| ((i as f32).powi(2) * 1e-7).sin()).collect()
    let f = 1760.0;
    let d = f / 44_100.0 * 2.0 * std::f64::consts::PI;
    (0..len).map(|i| {
        let i = i as f64;
        let amp = 100.0 * (i * -4e-5).exp();
        let tone = (i * d).sin() * amp;
        //tone.max(-1.0).min(1.0) as f32
        (tone / (1.0 + tone * tone).sqrt()) as f32
        //tone.tanh() as f32
    }).collect()
}

fn main() {
    let audio = gen_audio(441_000);
    let mut spect = Spect::new(1024);
    let (width, height) = spect.image_dims(audio.len());
    let img = spect.generate(&audio);

    let path = Path::new("foo.png");
    let f = File::create(path).unwrap();
    let w = BufWriter::new(f);

    let mut encoder = png::Encoder::new(w, width as u32, height as u32);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&img).unwrap();
}
