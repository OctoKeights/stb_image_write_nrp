//! Synthetic data, generated from a seed exactly as the C side of the comparison
//! (`compare/dump_stb_image_write.c`) generates it, and the dump that both the golden tests
//! and `examples/dump.rs` run.

// (The golden tests and the dump each use only part of this module.)
#![allow(dead_code)]

use std::fmt::Write;

use stb_image_write_nrp::{
    Channels, Image, PngFilter, WriteOptions, write_bmp, write_jpg, write_png, zlib_compress,
};

/// The two programs make the same blocks from the same seed: a 64-bit linear congruential
/// generator, the high half of its state.
pub struct Random {
    pub state: u64,
}

impl Random {
    pub fn next(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.state >> 32) as u32
    }
}

/// FNV-1a, 64 bits.
pub struct Hash {
    pub value: u64,
}

impl Hash {
    pub fn new() -> Hash {
        Hash {
            value: 14_695_981_039_346_656_037,
        }
    }

    pub fn add(&mut self, data: &[u8]) {
        for &byte in data {
            self.value ^= u64::from(byte);
            self.value = self.value.wrapping_mul(1_099_511_628_211);
        }
    }
}

const WORDS: [&str; 16] = [
    "the", "texture", "block", "of", "and", "compress", "a", "level", "mip", "to", "pixels", "in",
    "format", "is", "cube", "data",
];

/// Synthetic data of one of eight kinds, the same in both programs.
pub fn make_data(r: &mut Random) -> Vec<u8> {
    let len = match r.next() % 8 {
        0 => r.next() % 16,
        1 => r.next() % 300,
        2 | 3 => r.next() % 5000,
        4 | 5 => r.next() % 40000,
        6 => 60000 + r.next() % 20000,
        _ => r.next() % 200_000,
    } as usize;
    let kind = r.next() % 8;
    let mut buf = Vec::with_capacity(len);
    match kind {
        // noise
        0 => {
            for _ in 0..len {
                buf.push((r.next() >> 13) as u8);
            }
        }
        // four letters
        1 => {
            for _ in 0..len {
                buf.push(b'a' + (r.next() % 4) as u8);
            }
        }
        // words
        2 => {
            while buf.len() < len {
                let word = WORDS[(r.next() % 16) as usize];
                for &byte in word.as_bytes() {
                    if buf.len() < len {
                        buf.push(byte);
                    }
                }
                if buf.len() < len {
                    buf.push(b' ');
                }
            }
        }
        // runs
        3 => {
            while buf.len() < len {
                let byte = r.next() as u8;
                let run = 1 + r.next() % 300;
                for _ in 0..run {
                    if buf.len() < len {
                        buf.push(byte);
                    }
                }
            }
        }
        // copies of what came before
        4 => {
            while buf.len() < len {
                if buf.len() < 64 || r.next().is_multiple_of(4) {
                    buf.push((r.next() >> 9) as u8);
                } else {
                    let reach = buf.len().min(40000);
                    let dist = 1 + r.next() as usize % reach;
                    let n = 3 + r.next() % 260;
                    for _ in 0..n {
                        if buf.len() < len {
                            buf.push(buf[buf.len() - dist]);
                        }
                    }
                }
            }
        }
        // nothing
        5 => buf.resize(len, 0),
        // a slope
        6 => {
            for i in 0..len {
                buf.push((i / 7 + i % 13) as u8);
            }
        }
        // stretches of noise, of one value and of counting
        _ => {
            while buf.len() < len {
                let (stretch, what, value) = (1 + r.next() % 2000, r.next() % 3, r.next());
                for i in 0..stretch {
                    if buf.len() < len {
                        buf.push(match what {
                            0 => (r.next() >> 11) as u8,
                            1 => value as u8,
                            _ => value.wrapping_add(i) as u8,
                        });
                    }
                }
            }
        }
    }
    buf
}

/// A synthetic image of 8 bits a channel: its channels, its size, how far its rows are apart,
/// and its bytes, of one of seven kinds. What lies between the rows is 0xCD.
pub struct Picture {
    pub w: i32,
    pub h: i32,
    pub comp: i32,
    pub stride: usize,
    pub bytes: Vec<u8>,
}

fn wave(t: u32) -> u32 {
    let v = (t & 511) as i32 - 256;
    v.unsigned_abs().min(255)
}

pub fn make_picture(r: &mut Random) -> Picture {
    let comp = 1 + (r.next() % 4) as usize;
    let (w, h) = match r.next() % 6 {
        0 => (1 + r.next() % 4, 1 + r.next() % 4),
        1 | 2 => (1 + r.next() % 40, 1 + r.next() % 40),
        3 | 4 => (1 + r.next() % 100, 1 + r.next() % 100),
        _ => (1 + r.next() % 300, 1 + r.next() % 200),
    };
    let (w, h) = (w as usize, h as usize);
    let stride = w * comp
        + if r.next().is_multiple_of(3) {
            (r.next() % 7) as usize
        } else {
            0
        };
    let mut bytes = vec![0xCDu8; stride * h];
    let kind = r.next() % 7;
    let mut coefficient = [[0u32; 3]; 4];
    for c in &mut coefficient {
        *c = [r.next() % 64, r.next() % 64, r.next() % 4096];
    }
    let mut palette = [[0u32; 4]; 16];
    for colour in &mut palette {
        for c in colour {
            *c = r.next() & 255;
        }
    }
    let cell = 1 + r.next() % 12;
    let colours = 2 + r.next() % 4;
    let noise = 1 + r.next() % 32;
    for y in 0..h {
        for x in 0..w {
            let (ux, uy) = (x as u32, y as u32);
            let pick = if kind == 5 {
                r.next() % colours
            } else {
                ((ux / cell) * 7 + (uy / cell) * 13 + (ux / cell) * (uy / cell)) % 16
            } as usize;
            for c in 0..comp {
                let along = ux * coefficient[c][0] + uy * coefficient[c][1] + coefficient[c][2];
                let slope = along >> 4;
                let value = match kind {
                    0 => r.next() >> 13,           // noise
                    1 => palette[0][c],            // one colour
                    2 => slope,                    // a slope
                    3 | 5 => palette[pick][c],     // squares
                    4 => slope + r.next() % noise, // a slope with noise on it
                    // a few colours, anywhere
                    _ => wave(along), // waves
                };
                bytes[y * stride + x * comp + c] = value as u8;
            }
        }
    }
    Picture {
        w: w as i32,
        h: h as i32,
        comp: comp as i32,
        stride,
        bytes,
    }
}

impl Picture {
    /// The picture as the writers take it.
    pub fn image(&self) -> Image<'_> {
        Image::with_stride(
            &self.bytes,
            self.w as u32,
            self.h as u32,
            Channels::from_count(self.comp as u8).unwrap(),
            self.stride,
        )
        .unwrap()
    }
}

/// A PNG compression level as the C side numbers it, where anything below 5 counts as 5.
pub fn png_level(level: i32) -> u32 {
    level.max(0) as u32
}

/// A PNG filter as the C side numbers it: 0 to 4, or anything else for the best of each row.
pub fn png_filter(filter: i32) -> Option<PngFilter> {
    [
        PngFilter::None,
        PngFilter::Sub,
        PngFilter::Up,
        PngFilter::Average,
        PngFilter::Paeth,
    ]
    .get(usize::try_from(filter).ok()?)
    .copied()
}

/// A JPEG quality as the C side numbers it: 0 for 90, and below 1 counted as 1 (which, as
/// with 1, keeps the colour at half the size).
pub fn jpg_quality(quality: i32) -> u8 {
    if quality < 0 {
        1
    } else {
        quality.min(255) as u8
    }
}

/// The `stb_image_write` dump: synthetic images written as `.png` (with random compression
/// levels and filters), as `.bmp` and as `.jpg` (with random qualities), some upside down;
/// and for every fourth, synthetic data through the file's own zlib compressor.
pub fn stb_image_write(seed: u64, count: u32) -> String {
    let mut r = Random { state: seed };
    let mut hash = Hash::new();
    let (mut png_total, mut bmp_total, mut jpg_total, mut zlib_total, mut pixel_total) =
        (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut out = String::new();
    for i in 0..count {
        let p = make_picture(&mut r);
        pixel_total += p.w as u64 * p.h as u64;

        // The three settings, as the C side has them.
        let options = WriteOptions {
            png_compression_level: if r.next().is_multiple_of(4) {
                png_level((r.next() % 40) as i32 - 3)
            } else {
                8
            },
            png_filter: if r.next().is_multiple_of(3) {
                png_filter((r.next() % 8) as i32 - 1)
            } else {
                None
            },
            flip_vertically: r.next().is_multiple_of(4),
        };
        let image = p.image();

        let png = write_png(&image, &options);
        hash.add(&(png.len() as u32).to_le_bytes());
        hash.add(&png);
        png_total += png.len() as u64;

        let bmp = write_bmp(&image, &options);
        hash.add(&(bmp.len() as u32).to_le_bytes());
        hash.add(&bmp);
        bmp_total += bmp.len() as u64;

        // Nothing for the one it stands for, in range, near where the colour stops being
        // halved, and out of range.
        let quality = match r.next() % 4 {
            0 => 0,
            1 => 1 + (r.next() % 100) as i32,
            2 => 85 + (r.next() % 16) as i32,
            _ => (r.next() % 130) as i32 - 10,
        };
        let jpg = write_jpg(&image, jpg_quality(quality), &options);
        hash.add(&(jpg.len() as u32).to_le_bytes());
        hash.add(&jpg);
        jpg_total += jpg.len() as u64;

        if i % 4 == 0 {
            let data = make_data(&mut r);
            let level = if r.next().is_multiple_of(4) {
                (r.next() % 50) as i32
            } else {
                8
            };
            let packed = zlib_compress(&data, level as u32);
            hash.add(&(packed.len() as u32).to_le_bytes());
            hash.add(&packed);
            zlib_total += packed.len() as u64;
        }

        if (i + 1) % 200 == 0 || i + 1 == count {
            writeln!(out, "stb_image_write {} {:016x}", i + 1, hash.value).unwrap();
        }
    }
    out + &format!(
        "stb_image_write pixels: {pixel_total}; bytes of .png: {png_total}, .bmp: {bmp_total}, .jpg: {jpg_total}, zlib: {zlib_total}\n"
    )
}
