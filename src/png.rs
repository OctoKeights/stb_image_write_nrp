//! The PNG writer (`stbi_write_png_to_mem`).

use crate::tables::CRC_TABLE;
use crate::{Channels, Image, PngFilter, WriteOptions, zlib_compress};

/// The bytes of a `.png` file of `image`, 8 bits a channel (`stbi_write_png_to_mem`).
///
/// Each row is filtered with [`WriteOptions::png_filter`], or, by default, with whichever of
/// the five filters leaves the smallest bytes, and the rows are compressed with
/// [`zlib_compress`] at [`WriteOptions::png_compression_level`].
pub fn write_png(image: &Image, options: &WriteOptions) -> Vec<u8> {
    let n = image.channels.count();
    let row_len = image.width as usize * n;
    let height = image.height as usize;

    // Each row behind the byte that names its filter.
    let mut filtered = vec![0u8; (row_len + 1) * height];
    let mut candidate = vec![0u8; row_len];
    for (y, out) in filtered.chunks_exact_mut(row_len + 1).enumerate() {
        let row = image.file_row(y, options.flip_vertically);
        let up = (y > 0).then(|| image.file_row(y - 1, options.flip_vertically));
        let (filter_byte, line) = out.split_first_mut().unwrap();
        let filter = match options.png_filter {
            Some(filter) => {
                filter_row(line, row, up, n, filter);
                filter
            }
            None => {
                // The filter whose bytes, as signed values, sum smallest; the first on a tie.
                let mut best = (PngFilter::None, i32::MAX);
                for filter in [
                    PngFilter::None,
                    PngFilter::Sub,
                    PngFilter::Up,
                    PngFilter::Average,
                    PngFilter::Paeth,
                ] {
                    filter_row(&mut candidate, row, up, n, filter);
                    let estimate = candidate.iter().fold(0i32, |sum, &byte| {
                        sum.wrapping_add(i32::from(byte as i8).abs())
                    });
                    if estimate < best.1 {
                        best = (filter, estimate);
                        line.copy_from_slice(&candidate);
                    }
                }
                best.0
            }
        };
        *filter_byte = filter as u8;
    }
    let zlib = zlib_compress(&filtered, options.png_compression_level);
    drop(filtered);

    let mut out = Vec::with_capacity(8 + 12 + 13 + 12 + zlib.len() + 12);
    out.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);
    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&image.width.to_be_bytes());
    header[4..8].copy_from_slice(&image.height.to_be_bytes());
    header[8] = 8; // bits a channel
    header[9] = match image.channels {
        Channels::Grey => 0,
        Channels::GreyAlpha => 4,
        Channels::Rgb => 2,
        Channels::Rgba => 6,
    };
    write_chunk(&mut out, b"IHDR", &header);
    write_chunk(&mut out, b"IDAT", &zlib);
    write_chunk(&mut out, b"IEND", &[]);
    out
}

/// A chunk: its length, its name and its bytes, and the checksum of the last two.
fn write_chunk(out: &mut Vec<u8>, name: &[u8; 4], bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    let from = out.len();
    out.extend_from_slice(name);
    out.extend_from_slice(bytes);
    let crc = crc32(&out[from..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// The CRC-32 of `bytes`, as a PNG's chunks have it (`stbiw__crc32`).
fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(!0u32, |crc, &byte| {
        (crc >> 8) ^ CRC_TABLE[usize::from(byte ^ crc as u8)]
    })
}

/// Filters `row`, of pixels of `n` bytes, into `out` (`stbiw__encode_png_line`). `up` is the
/// row above in the file; on the first row, which has none, it counts as zeros.
fn filter_row(out: &mut [u8], row: &[u8], up: Option<&[u8]>, n: usize, filter: PngFilter) {
    let Some(up) = up else {
        // (The original's first-row variants: Up is None, and Average and Paeth see only the
        // left neighbour.)
        match filter {
            PngFilter::None | PngFilter::Up => out.copy_from_slice(row),
            PngFilter::Sub | PngFilter::Paeth => {
                out[..n].copy_from_slice(&row[..n]);
                for i in n..row.len() {
                    out[i] = row[i].wrapping_sub(row[i - n]);
                }
            }
            PngFilter::Average => {
                out[..n].copy_from_slice(&row[..n]);
                for i in n..row.len() {
                    out[i] = row[i].wrapping_sub(row[i - n] >> 1);
                }
            }
        }
        return;
    };
    match filter {
        PngFilter::None => out.copy_from_slice(row),
        PngFilter::Sub => {
            out[..n].copy_from_slice(&row[..n]);
            for i in n..row.len() {
                out[i] = row[i].wrapping_sub(row[i - n]);
            }
        }
        PngFilter::Up => {
            for ((out, &byte), &above) in out.iter_mut().zip(row).zip(up) {
                *out = byte.wrapping_sub(above);
            }
        }
        PngFilter::Average => {
            for i in 0..n {
                out[i] = row[i].wrapping_sub(up[i] >> 1);
            }
            for i in n..row.len() {
                let average = (u16::from(row[i - n]) + u16::from(up[i])) >> 1;
                out[i] = row[i].wrapping_sub(average as u8);
            }
        }
        PngFilter::Paeth => {
            for i in 0..n {
                out[i] = row[i].wrapping_sub(up[i]);
            }
            for i in n..row.len() {
                out[i] = row[i].wrapping_sub(paeth(row[i - n], up[i], up[i - n]));
            }
        }
    }
}

/// Whichever of `a` (left), `b` (up) and `c` (up-left) is nearest `a + b - c`, preferring
/// them in that order (`stbiw__paeth`).
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (a16, b16, c16) = (i16::from(a), i16::from(b), i16::from(c));
    let p = a16 + b16 - c16;
    let (pa, pb, pc) = ((p - a16).abs(), (p - b16).abs(), (p - c16).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miniz_nrp as miniz;

    fn image(x: usize, y: usize, comp: usize) -> Vec<u8> {
        (0..x * y * comp)
            .map(|i| ((i / comp % x) * 9 + (i / comp / x) * 5 + i % comp * 60) as u8)
            .collect()
    }

    #[test]
    fn a_png_has_its_three_chunks() {
        let options = WriteOptions::default();
        for count in 1..=4u8 {
            let channels = Channels::from_count(count).unwrap();
            let comp = usize::from(count);
            let pixels = image(13, 7, comp);
            let file = write_png(&Image::new(&pixels, 13, 7, channels).unwrap(), &options);
            assert_eq!(&file[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
            assert_eq!(&file[8..16], &[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
            assert_eq!(
                &file[16..29],
                &[0, 0, 0, 13, 0, 0, 0, 7, 8, [0, 4, 2, 6][comp - 1], 0, 0, 0]
            );
            assert_eq!(
                &file[file.len() - 12..],
                &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82]
            );
            // The rows come back out of the compressed chunk, each behind its filter.
            let length = u32::from_be_bytes(file[33..37].try_into().unwrap()) as usize;
            assert_eq!(&file[37..41], b"IDAT");
            let rows = miniz::decompress(&file[41..41 + length], 7 * (13 * comp + 1)).unwrap();
            assert_eq!(rows.len(), 7 * (13 * comp + 1));
            // Unfiltered, they are the pixels.
            let plain = write_png(
                &Image::new(&pixels, 13, 7, channels).unwrap(),
                &WriteOptions {
                    png_filter: Some(PngFilter::None),
                    ..options
                },
            );
            let length = u32::from_be_bytes(plain[33..37].try_into().unwrap()) as usize;
            let rows = miniz::decompress(&plain[41..41 + length], 7 * (13 * comp + 1)).unwrap();
            for (row, line) in rows.chunks(13 * comp + 1).enumerate() {
                assert_eq!(
                    (line[0], &line[1..]),
                    (0, &pixels[row * 13 * comp..(row + 1) * 13 * comp])
                );
            }
        }
        // A stride, and upside down.
        let pixels = image(5, 3, 3);
        let mut wide = vec![0xEEu8; 3 * 20];
        for row in 0..3 {
            wide[row * 20..row * 20 + 15].copy_from_slice(&pixels[row * 15..row * 15 + 15]);
        }
        let plain = Image::new(&pixels, 5, 3, Channels::Rgb).unwrap();
        assert_eq!(
            write_png(
                &Image::with_stride(&wide, 5, 3, Channels::Rgb, 20).unwrap(),
                &options
            ),
            write_png(&plain, &options)
        );
        let upside_down: Vec<u8> = pixels.chunks(15).rev().flatten().copied().collect();
        let flipped = WriteOptions {
            flip_vertically: true,
            ..options
        };
        assert_eq!(
            write_png(
                &Image::new(&upside_down, 5, 3, Channels::Rgb).unwrap(),
                &flipped
            ),
            write_png(&plain, &options)
        );
    }

    #[test]
    fn paeth_prefers_left_then_up() {
        assert_eq!(paeth(10, 20, 10), 20);
        assert_eq!(paeth(10, 10, 10), 10);
        assert_eq!(paeth(200, 50, 255), 50);
        assert_eq!(paeth(5, 9, 0), 9);
    }
}
