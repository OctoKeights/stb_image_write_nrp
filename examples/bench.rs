//! Throughput of the three writers on a synthetic 1024 by 768 image.
//!
//! ```sh
//! cargo run --release --example bench
//! ```
//!
//! Each figure is the best of several runs, in megapixels per second.
use stb_image_write_nrp::{Channels, Image, WriteOptions, write_bmp, write_jpg, write_png};
use std::time::Instant;

const RUNS: usize = 9;

/// A photograph-like RGBA image: smooth gradients with a little noise, and a few flat areas.
fn photo_like(w: usize, h: usize) -> Vec<u8> {
    let mut state = 1u64;
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let noise = (state >> 61) as usize;
            let flat = (x / 128 + y / 128) % 5 == 0;
            for c in 0..4 {
                out.push(if c == 3 {
                    255
                } else if flat {
                    (40 * c + 60) as u8
                } else {
                    ((x * (c + 1) + y * (3 - c)) / 4 + noise) as u8
                });
            }
        }
    }
    out
}

fn time(mut f: impl FnMut() -> usize) -> (f64, usize) {
    let mut best = f64::MAX;
    let mut len = 0;
    for _ in 0..RUNS {
        let start = Instant::now();
        len = f();
        best = best.min(start.elapsed().as_secs_f64());
    }
    (best, len)
}

fn main() {
    let (w, h) = (1024, 768);
    let rgba = photo_like(w, h);
    let rgb: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let options = WriteOptions::default();
    for (name, pixels, channels) in [
        ("rgb", &rgb, Channels::Rgb),
        ("rgba", &rgba, Channels::Rgba),
    ] {
        let image = Image::new(pixels, w as u32, h as u32, channels).unwrap();
        let megapixels = (w * h) as f64 / 1e6;
        let report = |what: &str, (t, len): (f64, usize)| {
            println!(
                "{name:5} {what:12} {:8.2} Mpixel/s   {len} bytes",
                megapixels / t
            )
        };
        report("png", time(|| write_png(&image, &options).len()));
        report("bmp", time(|| write_bmp(&image, &options).len()));
        report("jpg q90", time(|| write_jpg(&image, 90, &options).len()));
        report("jpg q95", time(|| write_jpg(&image, 95, &options).len()));
    }
}
