//! The JPEG writer (`stbi_write_jpg_core`), after Jon Olick's public-domain `jo_jpeg.cpp`
//! (<http://www.jonolick.com/code.html>).
//!
//! The colour conversion, the DCT and the quantisation are in single-precision floats, in the
//! original's order of operations, so that the coefficients round as the original's do.

use crate::tables::{
    STBIW_JPG_ZIGZAG as ZIGZAG, STD_AC_CHROMINANCE_NRCODES, STD_AC_CHROMINANCE_VALUES,
    STD_AC_LUMINANCE_NRCODES, STD_AC_LUMINANCE_VALUES, STD_DC_CHROMINANCE_NRCODES,
    STD_DC_CHROMINANCE_VALUES, STD_DC_LUMINANCE_NRCODES, STD_DC_LUMINANCE_VALUES, UVAC_HT, UVDC_HT,
    UVQT, YAC_HT, YDC_HT, YQT,
};
use crate::{Image, WriteOptions};

/// A Huffman code: each symbol's code and its length in bits.
type HuffmanTable = [[u16; 2]; 256];

/// The bytes of a baseline `.jpg` file of `image` (`stbi_write_jpg_to_func`).
///
/// `quality` goes from 1 (smallest) to 100 (best); 0 means 90, and above 100 counts as 100.
/// Up to 90, the colour is stored at half the resolution each way. Alpha is not written.
#[must_use]
pub fn write_jpg(image: &Image, quality: u8, options: &WriteOptions) -> Vec<u8> {
    // The quantisation as factors, with the DCT's scaling folded in.
    #[allow(clippy::excessive_precision)]
    const AASF: [f32; 8] = [
        1.0 * 2.828_427_125,
        1.387_039_845 * 2.828_427_125,
        1.306_562_965 * 2.828_427_125,
        1.175_875_602 * 2.828_427_125,
        1.0 * 2.828_427_125,
        0.785_694_958 * 2.828_427_125,
        0.541_196_100 * 2.828_427_125,
        0.275_899_379 * 2.828_427_125,
    ];
    let quality = if quality == 0 { 90 } else { i32::from(quality) };
    let subsample = quality <= 90;
    let quality = quality.clamp(1, 100);
    let scale = if quality < 50 {
        5000 / quality
    } else {
        200 - quality * 2
    };

    let (mut y_table, mut uv_table) = ([0u8; 64], [0u8; 64]);
    for i in 0..64 {
        let zigzag = usize::from(ZIGZAG[i]);
        y_table[zigzag] = ((YQT[i] * scale + 50) / 100).clamp(1, 255) as u8;
        uv_table[zigzag] = ((UVQT[i] * scale + 50) / 100).clamp(1, 255) as u8;
    }
    let (mut y_factors, mut uv_factors) = ([0.0f32; 64], [0.0f32; 64]);
    for k in 0..64 {
        let (row, col) = (k / 8, k % 8);
        let zigzag = usize::from(ZIGZAG[k]);
        y_factors[k] = 1.0 / (f32::from(y_table[zigzag]) * AASF[row] * AASF[col]);
        uv_factors[k] = 1.0 / (f32::from(uv_table[zigzag]) * AASF[row] * AASF[col]);
    }

    let mut bits = BitWriter::default();
    write_headers(&mut bits.out, image, subsample, &y_table, &uv_table);

    let luma = Component {
        factors: &y_factors,
        dc: &YDC_HT,
        ac: &YAC_HT,
        prev_dc: 0,
    };
    let chroma = Component {
        factors: &uv_factors,
        dc: &UVDC_HT,
        ac: &UVAC_HT,
        prev_dc: 0,
    };
    let (mut y, mut u, mut v) = (luma, chroma, chroma);
    let (width, height) = (image.width as usize, image.height as usize);
    if subsample {
        // Each 16 by 16 macroblock: four luma blocks, and one block of each chroma component
        // averaged from four pixels to one.
        for top in (0..height).step_by(16) {
            for left in (0..width).step_by(16) {
                let mut planes = Planes::<256>::of(image, options.flip_vertically, left, top);
                for at in [0, 8, 128, 136] {
                    y.encode(&mut bits, &mut planes.y[at..], 16);
                }
                let (mut sub_u, mut sub_v) = ([0.0f32; 64], [0.0f32; 64]);
                let (us, vs) = (&planes.cb, &planes.cr);
                for (pos, (su, sv)) in sub_u.iter_mut().zip(&mut sub_v).enumerate() {
                    let j = pos / 8 * 32 + pos % 8 * 2;
                    *su = (us[j] + us[j + 1] + us[j + 16] + us[j + 17]) * 0.25;
                    *sv = (vs[j] + vs[j + 1] + vs[j + 16] + vs[j + 17]) * 0.25;
                }
                u.encode(&mut bits, &mut sub_u, 8);
                v.encode(&mut bits, &mut sub_v, 8);
            }
        }
    } else {
        for top in (0..height).step_by(8) {
            for left in (0..width).step_by(8) {
                let mut planes = Planes::<64>::of(image, options.flip_vertically, left, top);
                y.encode(&mut bits, &mut planes.y, 8);
                u.encode(&mut bits, &mut planes.cb, 8);
                v.encode(&mut bits, &mut planes.cr, 8);
            }
        }
    }

    // Pad the last byte with ones, then the end-of-image marker.
    bits.write([0x7F, 7]);
    bits.out.extend_from_slice(&[0xFF, 0xD9]);
    bits.out
}

/// The headers, up to the start of the scan: JFIF, the quantisation tables, the frame, the
/// Huffman tables, and the scan's own header.
fn write_headers(
    out: &mut Vec<u8>,
    image: &Image,
    subsample: bool,
    y_table: &[u8; 64],
    uv_table: &[u8; 64],
) {
    const JFIF_AND_DQT: [u8; 25] = [
        0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
        0xFF, 0xDB, 0, 0x84, 0,
    ];
    const SOS: [u8; 14] = [0xFF, 0xDA, 0, 0xC, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 0x3F, 0];
    // (Sizes past 16 bits are written as their low 16 bits, as the original writes them.)
    let (width, height) = (image.width as u16, image.height as u16);
    out.extend_from_slice(&JFIF_AND_DQT);
    out.extend_from_slice(y_table);
    out.push(1);
    out.extend_from_slice(uv_table);
    // The frame: 8 bits, the size, three components; then the start of the Huffman tables.
    out.extend_from_slice(&[0xFF, 0xC0, 0, 0x11, 8]);
    out.extend_from_slice(&height.to_be_bytes());
    out.extend_from_slice(&width.to_be_bytes());
    let sampling = if subsample { 0x22 } else { 0x11 };
    out.extend_from_slice(&[3, 1, sampling, 0, 2, 0x11, 1, 3, 0x11, 1]);
    out.extend_from_slice(&[0xFF, 0xC4, 0x01, 0xA2, 0]);
    out.extend_from_slice(&STD_DC_LUMINANCE_NRCODES[1..]);
    out.extend_from_slice(&STD_DC_LUMINANCE_VALUES);
    out.push(0x10);
    out.extend_from_slice(&STD_AC_LUMINANCE_NRCODES[1..]);
    out.extend_from_slice(&STD_AC_LUMINANCE_VALUES);
    out.push(1);
    out.extend_from_slice(&STD_DC_CHROMINANCE_NRCODES[1..]);
    out.extend_from_slice(&STD_DC_CHROMINANCE_VALUES);
    out.push(0x11);
    out.extend_from_slice(&STD_AC_CHROMINANCE_NRCODES[1..]);
    out.extend_from_slice(&STD_AC_CHROMINANCE_VALUES);
    out.extend_from_slice(&SOS);
}

/// The Y, Cb and Cr of a square of `LEN` pixels (8 by 8, or 16 by 16), row by row.
struct Planes<const LEN: usize> {
    y: [f32; LEN],
    cb: [f32; LEN],
    cr: [f32; LEN],
}

impl<const LEN: usize> Planes<LEN> {
    /// The square from (`left`, `top`), the last row and column repeated past the image's
    /// edges.
    fn of(image: &Image, flip_vertically: bool, left: usize, top: usize) -> Self {
        let size = LEN.isqrt();
        let (width, height) = (image.width as usize, image.height as usize);
        let channels = image.channels.count();
        // Grey with or without alpha has the one value for all three.
        let (green_at, blue_at) = if channels > 2 { (1, 2) } else { (0, 0) };
        let mut planes = Self {
            y: [0.0; LEN],
            cb: [0.0; LEN],
            cr: [0.0; LEN],
        };
        let mut pos = 0;
        for y in top..top + size {
            let row = image.file_row(y.min(height - 1), flip_vertically);
            for x in left..left + size {
                let at = x.min(width - 1) * channels;
                let (r, g, b) = (
                    f32::from(row[at]),
                    f32::from(row[at + green_at]),
                    f32::from(row[at + blue_at]),
                );
                planes.y[pos] = 0.29900 * r + 0.58700 * g + 0.11400 * b - 128.0;
                planes.cb[pos] = -0.16874 * r - 0.33126 * g + 0.50000 * b;
                planes.cr[pos] = 0.50000 * r - 0.41869 * g - 0.08131 * b;
                pos += 1;
            }
        }
        planes
    }
}

/// One colour component's quantisation and Huffman codes, and the DC value of its last block.
#[derive(Clone, Copy)]
struct Component<'a> {
    factors: &'a [f32; 64],
    dc: &'a HuffmanTable,
    ac: &'a HuffmanTable,
    prev_dc: i32,
}

impl Component<'_> {
    /// Transforms, quantises and writes the 8 by 8 block at the start of `block`, whose rows
    /// are `stride` values apart (`stbiw__jpg_processDU`).
    fn encode(&mut self, bits: &mut BitWriter, block: &mut [f32], stride: usize) {
        for row in 0..8 {
            dct(block, row * stride, 1);
        }
        for col in 0..8 {
            dct(block, col, stride);
        }
        let mut du = [0i32; 64];
        let (factors, _) = self.factors.as_chunks::<8>();
        let (zigzag, _) = ZIGZAG.as_chunks::<8>();
        for (y, (factors, zigzag)) in factors.iter().zip(zigzag).enumerate() {
            let row: &[f32; 8] = block[y * stride..y * stride + 8].try_into().unwrap();
            for ((&value, &factor), &to) in row.iter().zip(factors).zip(zigzag) {
                let v = value * factor;
                // Rounded half away from zero, by truncation, as the original does: its
                // `v < 0 ? v - 0.5 : v + 0.5` without a branch. (For -0.0 this adds -0.5 where
                // the original adds 0.5; both truncate to 0.)
                du[usize::from(to)] = (v + 0.5f32.copysign(v)) as i32;
            }
        }

        // The DC coefficient, as the difference from the last block's.
        let diff = du[0] - self.prev_dc;
        self.prev_dc = du[0];
        if diff == 0 {
            bits.write(self.dc[0]);
        } else {
            let value = magnitude(diff);
            bits.write(self.dc[usize::from(value[1])]);
            bits.write(value);
        }

        // The AC coefficients, as runs of zeros and the values between them.
        let (eob, sixteen_zeros) = (self.ac[0x00], self.ac[0xF0]);
        let Some(last) = du[1..].iter().rposition(|&c| c != 0).map(|i| i + 1) else {
            bits.write(eob);
            return;
        };
        let mut i = 1;
        while i <= last {
            let start = i;
            while du[i] == 0 {
                i += 1;
            }
            let mut zeros = i - start;
            for _ in 0..zeros >> 4 {
                bits.write(sixteen_zeros);
            }
            zeros &= 15;
            let value = magnitude(du[i]);
            bits.write(self.ac[(zeros << 4) + usize::from(value[1])]);
            bits.write(value);
            i += 1;
        }
        if last != 63 {
            bits.write(eob);
        }
    }
}

/// A coefficient's bits and their count, as JPEG codes them: negative values as their one's
/// complement (`stbiw__jpg_calcBits`).
fn magnitude(value: i32) -> [u16; 2] {
    let len = (32 - value.unsigned_abs().leading_zeros()).max(1);
    let bits = if value < 0 { value - 1 } else { value };
    [(bits & ((1 << len) - 1)) as u16, len as u16]
}

/// The forward DCT of eight values of `d`, `step` apart from `at`, in place (AAN, as in the
/// IJG's `jfdctflt.c`; `stbiw__jpg_DCT`).
// (The constants are the published ones, to the digits the C has, and the names are the
// algorithm's: `tmp0` to `tmp13` and `z1` to `z13`.)
#[allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::similar_names
)]
fn dct(d: &mut [f32], at: usize, step: usize) {
    let i = |k: usize| at + step * k;
    let (d0, d1, d2, d3) = (d[i(0)], d[i(1)], d[i(2)], d[i(3)]);
    let (d4, d5, d6, d7) = (d[i(4)], d[i(5)], d[i(6)], d[i(7)]);

    let tmp0 = d0 + d7;
    let tmp7 = d0 - d7;
    let tmp1 = d1 + d6;
    let tmp6 = d1 - d6;
    let tmp2 = d2 + d5;
    let tmp5 = d2 - d5;
    let tmp3 = d3 + d4;
    let tmp4 = d3 - d4;

    // The even part.
    let tmp10 = tmp0 + tmp3;
    let tmp13 = tmp0 - tmp3;
    let tmp11 = tmp1 + tmp2;
    let tmp12 = tmp1 - tmp2;
    let z1 = (tmp12 + tmp13) * 0.707_106_781; // c4
    d[i(0)] = tmp10 + tmp11;
    d[i(4)] = tmp10 - tmp11;
    d[i(2)] = tmp13 + z1;
    d[i(6)] = tmp13 - z1;

    // The odd part.
    let tmp10 = tmp4 + tmp5;
    let tmp11 = tmp5 + tmp6;
    let tmp12 = tmp6 + tmp7;
    // (The rotator is modified from figure 4-8 of Pennebaker and Mitchell to avoid extra
    // negations.)
    let z5 = (tmp10 - tmp12) * 0.382_683_433; // c6
    let z2 = tmp10 * 0.541_196_100 + z5; // c2 - c6
    let z4 = tmp12 * 1.306_562_965 + z5; // c2 + c6
    let z3 = tmp11 * 0.707_106_781; // c4
    let z11 = tmp7 + z3;
    let z13 = tmp7 - z3;
    d[i(5)] = z13 + z2;
    d[i(3)] = z13 - z2;
    d[i(1)] = z11 + z4;
    d[i(7)] = z11 - z4;
}

/// The scan, written a bit at a time, most significant first, with a zero byte after every
/// 0xFF.
#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    buffer: u32,
    count: u32,
}

impl BitWriter {
    /// Writes a code: its bits, and their count (`stbiw__jpg_writeBits`).
    #[inline]
    fn write(&mut self, [bits, len]: [u16; 2]) {
        self.count += u32::from(len);
        self.buffer |= u32::from(bits) << (24 - self.count);
        while self.count >= 8 {
            let byte = (self.buffer >> 16) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
            self.buffer <<= 8;
            self.count -= 8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Channels;

    #[test]
    fn a_jpg_has_its_markers() {
        let pixels: Vec<u8> = (0..20 * 11 * 3).map(|i| (i * 7 % 251) as u8).collect();
        let image = Image::new(&pixels, 20, 11, Channels::Rgb).unwrap();
        let options = WriteOptions::default();
        for (quality, sampling) in [(0, 0x22), (50, 0x22), (95, 0x11)] {
            let file = write_jpg(&image, quality, &options);
            assert_eq!(&file[..4], &[0xFF, 0xD8, 0xFF, 0xE0]);
            assert_eq!(&file[file.len() - 2..], &[0xFF, 0xD9]);
            // The frame header: 8 bits, 11 by 20, three components, the first sampled as said.
            let frame = 25 + 64 + 1 + 64;
            assert_eq!(
                &file[frame..frame + 12],
                &[0xFF, 0xC0, 0, 0x11, 8, 0, 11, 0, 20, 3, 1, sampling]
            );
        }
        // Better quality is more bytes.
        assert!(write_jpg(&image, 10, &options).len() < write_jpg(&image, 100, &options).len());
    }

    #[test]
    fn magnitudes_are_coded_as_jpeg_has_them() {
        assert_eq!(magnitude(1), [1, 1]);
        assert_eq!(magnitude(-1), [0, 1]);
        assert_eq!(magnitude(5), [5, 3]);
        assert_eq!(magnitude(-5), [2, 3]);
        assert_eq!(magnitude(1023), [1023, 10]);
    }
}
