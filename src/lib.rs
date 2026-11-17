//! Images to `.png`, `.bmp` and `.jpg` files: a native Rust port of Sean Barrett's
//! [`stb_image_write.h`](https://github.com/nothings/stb) v1.18, three of its writers. Each
//! gives the bytes the original writes.
//!
//! - [`write_png`]: 8 bits a channel, one to four channels (grey, grey with alpha, RGB, RGBA),
//!   with the original's own small zlib compressor ([`zlib_compress`]).
//! - [`write_bmp`]: 24 bits a pixel, or 32 with alpha when there are four channels.
//! - [`write_jpg`]: baseline JPEG, after Jon Olick's `jo_jpeg.cpp`.
//!
//! The original keeps three settings in globals; here they are a [`WriteOptions`] handed to
//! each call. Its `.tga` and `.hdr` writers are not ported, and nor are its ways of writing
//! through a callback or straight to a file: each function here gives back the file's bytes.

#![forbid(unsafe_code)]
// The code follows the original's line for line, and these are its shapes.
#![allow(
    clippy::needless_range_loop,
    clippy::too_many_arguments,
    clippy::manual_range_contains
)]

mod tables;

use tables::*;

/// `stbiw__crc32`: the CRC-32 of `buffer`, as a PNG's chunks have it.
fn crc32(buffer: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in buffer {
        crc = (crc >> 8) ^ CRC_TABLE[(byte ^ (crc & 0xff) as u8) as usize];
    }
    !crc
}

/// The settings the original keeps in globals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriteOptions {
    /// How hard the PNG writer's compressor tries: `stbi_write_png_compression_level`, 8
    /// unless set. Below 5 counts as 5.
    pub png_compression_level: i32,
    /// The filter the PNG writer gives every row, 0 to 4, or -1 (and 5 and above) to try all
    /// five on each row and keep the best: `stbi_write_force_png_filter`, -1 unless set.
    pub force_png_filter: i32,
    /// Write the image upside down: `stbi_flip_vertically_on_write`.
    pub flip_vertically_on_write: bool,
}

impl Default for WriteOptions {
    fn default() -> WriteOptions {
        WriteOptions {
            png_compression_level: 8,
            force_png_filter: -1,
            flip_vertically_on_write: false,
        }
    }
}

/// `stbiw__validate_dimensions`: does some boilerplate validation on image dimensions that is
/// the same for all formats. it's intended to make sure dimensions are sane and avoid overflow
/// issues.
fn stbiw_validate_dimensions(x: i32, y: i32, comp: i32) -> bool {
    if !(1..=4).contains(&comp) {
        return false;
    }
    // limit x to [1,INT_MAX/4] to exclude negative or 0 dimensions.
    // INT_MAX/4 because it guarantees that x*comp <= INT_MAX, and that's our usual offset calc.
    if x <= 0 || x > i32::MAX / 4 {
        return false;
    }
    // same bounds on y for symmetry
    if y <= 0 || y > i32::MAX / 4 {
        return false;
    }
    true
}

/// Whether `data` holds `y` rows of `x` pixels of `comp` bytes, `stride` bytes from one row to
/// the next. (The original trusts its caller; here a slice that is too short is refused.)
fn holds(data: &[u8], x: i32, y: i32, comp: i32, stride: usize) -> bool {
    let row = x as usize * comp as usize;
    stride >= row
        && (y as usize - 1)
            .checked_mul(stride)
            .and_then(|before| before.checked_add(row))
            .is_some_and(|needed| data.len() >= needed)
}

// ---------------------------------------------------------------------------------------------
// BMP

/// `stbiw__write_pixel`
fn stbiw_write_pixel(
    out: &mut Vec<u8>,
    rgb_dir: i32,
    comp: i32,
    write_alpha: i32,
    expand_mono: bool,
    d: &[u8],
) {
    let bg = [255i32, 0, 255];
    if write_alpha < 0 {
        out.push(d[comp as usize - 1]);
    }
    match comp {
        // 2 pixels = mono + alpha, alpha is written separately, so same as 1-channel case
        1 | 2 => {
            if expand_mono {
                out.extend_from_slice(&[d[0], d[0], d[0]]); // monochrome bmp
            } else {
                out.push(d[0]); // monochrome TGA
            }
        }
        4 if write_alpha == 0 => {
            // composite against pink background
            let mut px = [0u8; 3];
            for k in 0..3 {
                px[k] = (bg[k] + ((i32::from(d[k]) - bg[k]) * i32::from(d[3])) / 255) as u8;
            }
            out.extend_from_slice(&[
                px[(1 - rgb_dir) as usize],
                px[1],
                px[(1 + rgb_dir) as usize],
            ]);
        }
        3 | 4 => {
            out.extend_from_slice(&[d[(1 - rgb_dir) as usize], d[1], d[(1 + rgb_dir) as usize]])
        }
        _ => {}
    }
    if write_alpha > 0 {
        out.push(d[comp as usize - 1]);
    }
}

/// `stbiw__write_pixels`
fn stbiw_write_pixels(
    out: &mut Vec<u8>,
    rgb_dir: i32,
    mut vdir: i32,
    x: i32,
    y: i32,
    comp: i32,
    data: &[u8],
    stride_in_bytes: usize,
    write_alpha: i32,
    scanline_pad: usize,
    expand_mono: bool,
    options: &WriteOptions,
) {
    if !stbiw_validate_dimensions(x, y, comp) {
        return;
    }

    if options.flip_vertically_on_write {
        vdir *= -1;
    }

    let (mut j, j_end) = if vdir < 0 { (y - 1, -1) } else { (0, y) };

    while j != j_end {
        for i in 0..x as usize {
            let at = j as usize * stride_in_bytes + i * comp as usize;
            stbiw_write_pixel(
                out,
                rgb_dir,
                comp,
                write_alpha,
                expand_mono,
                &data[at..at + comp as usize],
            );
        }
        out.extend_from_slice(&[0u8; 4][..scanline_pad]);
        j += vdir;
    }
}

fn put_u16(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&(value as u16).to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// The bytes of a `.bmp` file for an image: `stbi_write_bmp_core`. `data` holds `y` rows of `x`
/// pixels from the top, each of `comp` bytes (1 grey, 2 grey and alpha, 3 RGB, 4 RGBA), a row
/// every `stride_in_bytes` bytes (0 for rows that follow one another).
///
/// One to three channels are written as 24 bits a pixel (grey as three equal values; the alpha
/// of two channels is dropped), four as 32 bits with alpha.
///
/// None when the size is not above zero, `comp` is not 1 to 4, or `data` is too short. (The
/// original writes a header with no pixels for such a size, and calls it done.)
pub fn write_bmp(
    x: i32,
    y: i32,
    comp: i32,
    data: &[u8],
    stride_in_bytes: usize,
    options: &WriteOptions,
) -> Option<Vec<u8>> {
    if !stbiw_validate_dimensions(x, y, comp) {
        return None;
    }
    let stride_in_bytes = if stride_in_bytes == 0 {
        x as usize * comp as usize
    } else {
        stride_in_bytes
    };
    if !holds(data, x, y, comp, stride_in_bytes) {
        return None;
    }
    let mut out = Vec::new();
    if comp != 4 {
        // write RGB bitmap
        let pad = (-x * 3) & 3;
        // file header
        out.extend_from_slice(b"BM");
        put_u32(&mut out, (14 + 40 + (x * 3 + pad) * y) as u32);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0);
        put_u32(&mut out, 14 + 40);
        // bitmap header
        put_u32(&mut out, 40);
        put_u32(&mut out, x as u32);
        put_u32(&mut out, y as u32);
        put_u16(&mut out, 1);
        put_u16(&mut out, 24);
        for _ in 0..6 {
            put_u32(&mut out, 0);
        }
        stbiw_write_pixels(
            &mut out,
            -1,
            -1,
            x,
            y,
            comp,
            data,
            stride_in_bytes,
            0,
            pad as usize,
            true,
            options,
        );
    } else {
        // RGBA bitmaps need a v4 header
        // use BI_BITFIELDS mode with 32bpp and alpha mask
        // (straight BI_RGB with alpha mask doesn't work in most readers)
        // file header
        out.extend_from_slice(b"BM");
        put_u32(&mut out, (14 + 108 + x * y * 4) as u32);
        put_u16(&mut out, 0);
        put_u16(&mut out, 0);
        put_u32(&mut out, 14 + 108);
        // bitmap V4 header
        put_u32(&mut out, 108);
        put_u32(&mut out, x as u32);
        put_u32(&mut out, y as u32);
        put_u16(&mut out, 1);
        put_u16(&mut out, 32);
        for value in [
            3,
            0,
            0,
            0,
            0,
            0,
            0xff0000,
            0xff00,
            0xff,
            0xff000000u32,
            0x73524742,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ] {
            put_u32(&mut out, value);
        }
        stbiw_write_pixels(
            &mut out,
            -1,
            -1,
            x,
            y,
            comp,
            data,
            stride_in_bytes,
            1,
            0,
            true,
            options,
        );
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// PNG writer

/// `stbiw__zlib_bitrev`
fn stbiw_zlib_bitrev(mut code: i32, codebits: i32) -> i32 {
    let mut res = 0;
    for _ in 0..codebits {
        res = (res << 1) | (code & 1);
        code >>= 1;
    }
    res
}

/// `stbiw__zlib_countm`
fn stbiw_zlib_countm(a: &[u8], b: &[u8], limit: i32) -> i32 {
    let mut i = 0;
    while i < limit && i < 258 {
        if a[i as usize] != b[i as usize] {
            break;
        }
        i += 1;
    }
    i
}

/// `stbiw__zhash`
fn stbiw_zhash(data: &[u8]) -> u32 {
    let mut hash = u32::from(data[0]) + (u32::from(data[1]) << 8) + (u32::from(data[2]) << 16);
    hash ^= hash << 3;
    hash = hash.wrapping_add(hash >> 5);
    hash ^= hash << 4;
    hash = hash.wrapping_add(hash >> 17);
    hash ^= hash << 25;
    hash = hash.wrapping_add(hash >> 6);
    hash
}

const STBIW_ZHASH: usize = 16384;

/// The bits on their way out of [`zlib_compress`].
struct ZlibOut {
    out: Vec<u8>,
    bitbuf: u32,
    bitcount: i32,
}

impl ZlibOut {
    /// `stbiw__zlib_add`
    fn add(&mut self, code: i32, codebits: i32) {
        self.bitbuf |= (code as u32) << self.bitcount;
        self.bitcount += codebits;
        // stbiw__zlib_flushf
        while self.bitcount >= 8 {
            self.out.push(self.bitbuf as u8);
            self.bitbuf >>= 8;
            self.bitcount -= 8;
        }
    }

    /// `stbiw__zlib_huffa`
    fn huffa(&mut self, b: i32, c: i32) {
        self.add(stbiw_zlib_bitrev(b, c), c);
    }

    // default huffman tables
    /// `stbiw__zlib_huff`
    fn huff(&mut self, n: i32) {
        if n <= 143 {
            self.huffa(0x30 + n, 8);
        } else if n <= 255 {
            self.huffa(0x190 + n - 144, 9);
        } else if n <= 279 {
            self.huffa(n - 256, 7);
        } else {
            self.huffa(0xc0 + n - 280, 8);
        }
    }

    /// `stbiw__zlib_huffb`
    fn huffb(&mut self, n: i32) {
        if n <= 143 {
            self.huffa(0x30 + n, 8);
        } else {
            self.huffa(0x190 + n - 144, 9);
        }
    }
}

/// Compresses some bytes to a zlib stream, the small way of the original's PNG writer: one
/// block with the fixed codes, or the bytes as they are when that would be larger:
/// `stbi_zlib_compress`. `quality` is how many earlier places are kept for each run of three
/// bytes; below 5 counts as 5.
pub fn zlib_compress(data: &[u8], quality: i32) -> Vec<u8> {
    static LENGTHC: [u16; 30] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258, 259,
    ];
    static LENGTHEB: [u8; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    static DISTC: [u16; 31] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 32768,
    ];
    static DISTEB: [u8; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    let data_len = data.len() as i32;
    let mut z = ZlibOut {
        out: Vec::new(),
        bitbuf: 0,
        bitcount: 0,
    };
    // (For each hash, the places in the data where bytes of that hash begin.)
    let mut hash_table: Vec<Vec<i32>> = vec![Vec::new(); STBIW_ZHASH];
    let quality = quality.max(5);

    z.out.push(0x78); // DEFLATE 32K window
    z.out.push(0x5e); // FLEVEL = 1
    z.add(1, 1); // BFINAL = 1
    z.add(1, 2); // BTYPE = 1 -- fixed huffman

    let mut i = 0i32;
    while i < data_len - 3 {
        // hash next 3 bytes of data to be compressed
        let mut h = stbiw_zhash(&data[i as usize..]) as usize & (STBIW_ZHASH - 1);
        let mut best = 3;
        let mut bestloc: Option<i32> = None;
        for &place in &hash_table[h] {
            if place > i - 32768 {
                // if entry lies within window
                let d =
                    stbiw_zlib_countm(&data[place as usize..], &data[i as usize..], data_len - i);
                if d >= best {
                    best = d;
                    bestloc = Some(place);
                }
            }
        }
        // when hash table entry is too long, delete half the entries
        if hash_table[h].len() == 2 * quality as usize {
            hash_table[h].drain(..quality as usize);
        }
        hash_table[h].push(i);

        if bestloc.is_some() {
            // "lazy matching" - check match at *next* byte,
            // and if it's better, do cur byte as literal
            h = stbiw_zhash(&data[i as usize + 1..]) as usize & (STBIW_ZHASH - 1);
            for &place in &hash_table[h] {
                if place > i - 32767 {
                    let e = stbiw_zlib_countm(
                        &data[place as usize..],
                        &data[i as usize + 1..],
                        data_len - i - 1,
                    );
                    if e > best {
                        // if next match is better, bail on current match
                        bestloc = None;
                        break;
                    }
                }
            }
        }

        if let Some(bestloc) = bestloc {
            let d = i - bestloc; // distance back
            debug_assert!(d <= 32767 && best <= 258);
            let mut j = 0;
            while best > i32::from(LENGTHC[j + 1]) - 1 {
                j += 1;
            }
            z.huff(j as i32 + 257);
            if LENGTHEB[j] != 0 {
                z.add(best - i32::from(LENGTHC[j]), i32::from(LENGTHEB[j]));
            }
            j = 0;
            while d > i32::from(DISTC[j + 1]) - 1 {
                j += 1;
            }
            z.add(stbiw_zlib_bitrev(j as i32, 5), 5);
            if DISTEB[j] != 0 {
                z.add(d - i32::from(DISTC[j]), i32::from(DISTEB[j]));
            }
            i += best;
        } else {
            z.huffb(i32::from(data[i as usize]));
            i += 1;
        }
    }
    // write out final bytes
    while i < data_len {
        z.huffb(i32::from(data[i as usize]));
        i += 1;
    }
    z.huff(256); // end of block
    // pad with 0 bits to byte boundary
    while z.bitcount != 0 {
        z.add(0, 1);
    }

    let mut out = z.out;

    // store uncompressed instead if compression was worse
    if out.len() as i32 > data_len + 2 + ((data_len + 32766) / 32767) * 5 {
        out.truncate(2); // truncate to DEFLATE 32K window and FLEVEL = 1
        let mut j = 0;
        while j < data_len {
            let mut blocklen = data_len - j;
            if blocklen > 32767 {
                blocklen = 32767;
            }
            out.push(u8::from(data_len - j == blocklen)); // BFINAL = ?, BTYPE = 0 -- no compression
            out.push(blocklen as u8); // LEN
            out.push((blocklen >> 8) as u8);
            out.push(!blocklen as u8); // NLEN
            out.push((!blocklen >> 8) as u8);
            out.extend_from_slice(&data[j as usize..(j + blocklen) as usize]);
            j += blocklen;
        }
    }

    {
        // compute adler32 on input
        let (mut s1, mut s2) = (1u32, 0u32);
        let mut blocklen = data.len() % 5552;
        let mut j = 0;
        while j < data.len() {
            for &byte in &data[j..j + blocklen] {
                s1 += u32::from(byte);
                s2 += s1;
            }
            s1 %= 65521;
            s2 %= 65521;
            j += blocklen;
            blocklen = 5552;
        }
        out.push((s2 >> 8) as u8);
        out.push(s2 as u8);
        out.push((s1 >> 8) as u8);
        out.push(s1 as u8);
    }
    out
}

/// `stbiw__paeth`
fn stbiw_paeth(a: i32, b: i32, c: i32) -> u8 {
    let p = a + b - c;
    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
    if pa <= pb && pa <= pc {
        return a as u8;
    }
    if pb <= pc {
        return b as u8;
    }
    c as u8
}

// @OPTIMIZE: provide an option that always forces left-predict or paeth predict
/// `stbiw__encode_png_line`
fn stbiw_encode_png_line(
    pixels: &[u8],
    stride_bytes: usize,
    width: i32,
    height: i32,
    y: i32,
    n: i32,
    filter_type: i32,
    line_buffer: &mut [u8],
    flip: bool,
) {
    static MAPPING: [i32; 5] = [0, 1, 2, 3, 4];
    static FIRSTMAP: [i32; 5] = [0, 1, 0, 5, 6];
    let mymap = if y != 0 { &MAPPING } else { &FIRSTMAP };
    let filter = mymap[filter_type as usize];
    let n = n as usize;
    let len = width as usize * n;
    let row = if flip { height - 1 - y } else { y } as usize;
    let z = &pixels[row * stride_bytes..row * stride_bytes + len];

    if filter == 0 {
        line_buffer[..len].copy_from_slice(z);
        return;
    }

    // The row before this one in the file (`z[i - signed_stride]`); the first row asks for
    // none.
    let up: &[u8] = if y != 0 {
        let before = if flip { row + 1 } else { row - 1 };
        &pixels[before * stride_bytes..before * stride_bytes + len]
    } else {
        &[]
    };
    let int = |byte: u8| i32::from(byte);

    // first loop isn't optimized since it's just one pixel
    for i in 0..n {
        line_buffer[i] = match filter {
            1 => z[i],
            2 => z[i].wrapping_sub(up[i]),
            3 => z[i].wrapping_sub(up[i] >> 1),
            4 => z[i].wrapping_sub(stbiw_paeth(0, int(up[i]), 0)),
            _ => z[i],
        };
    }
    for i in n..len {
        line_buffer[i] = match filter {
            1 => z[i].wrapping_sub(z[i - n]),
            2 => z[i].wrapping_sub(up[i]),
            3 => z[i].wrapping_sub(((int(z[i - n]) + int(up[i])) >> 1) as u8),
            4 => z[i].wrapping_sub(stbiw_paeth(int(z[i - n]), int(up[i]), int(up[i - n]))),
            5 => z[i].wrapping_sub(z[i - n] >> 1),
            _ => z[i].wrapping_sub(stbiw_paeth(int(z[i - n]), 0, 0)),
        };
    }
}

/// The bytes of a `.png` file for an image: `stbi_write_png_to_mem`. `pixels` holds `y` rows
/// of `x` pixels from the top, each of `n` bytes (1 grey, 2 grey and alpha, 3 RGB, 4 RGBA), a
/// row every `stride_bytes` bytes (0 for rows that follow one another).
///
/// None when the size is not above zero, `n` is not 1 to 4, or `pixels` is too short.
pub fn write_png(
    pixels: &[u8],
    stride_bytes: usize,
    x: i32,
    y: i32,
    n: i32,
    options: &WriteOptions,
) -> Option<Vec<u8>> {
    let mut force_filter = options.force_png_filter;
    let ctype = [-1i32, 0, 4, 2, 6];
    let sig = [137u8, 80, 78, 71, 13, 10, 26, 10];

    if !stbiw_validate_dimensions(x, y, n) {
        return None;
    }

    let stride_bytes = if stride_bytes == 0 {
        x as usize * n as usize
    } else {
        stride_bytes
    };
    if !holds(pixels, x, y, n, stride_bytes) {
        return None;
    }

    if force_filter >= 5 {
        force_filter = -1;
    }

    let row = x as usize * n as usize;
    let mut filt = vec![0u8; (row + 1).checked_mul(y as usize)?];
    let mut line_buffer = vec![0u8; row];
    for j in 0..y {
        let filter_type;
        if force_filter > -1 {
            filter_type = force_filter;
            stbiw_encode_png_line(
                pixels,
                stride_bytes,
                x,
                y,
                j,
                n,
                force_filter,
                &mut line_buffer,
                options.flip_vertically_on_write,
            );
        } else {
            // Estimate the best filter by running through all of them:
            let (mut best_filter, mut best_filter_val) = (0, 0x7fffffff);
            for filter_type in 0..5 {
                stbiw_encode_png_line(
                    pixels,
                    stride_bytes,
                    x,
                    y,
                    j,
                    n,
                    filter_type,
                    &mut line_buffer,
                    options.flip_vertically_on_write,
                );

                // Estimate the entropy of the line using this filter; the less, the better.
                let mut est = 0i32;
                for &value in &line_buffer {
                    est = est.wrapping_add(i32::from(value as i8).abs());
                }
                if est < best_filter_val {
                    best_filter_val = est;
                    best_filter = filter_type;
                }
            }
            stbiw_encode_png_line(
                pixels,
                stride_bytes,
                x,
                y,
                j,
                n,
                best_filter,
                &mut line_buffer,
                options.flip_vertically_on_write,
            );
            filter_type = best_filter;
        }
        // when we get here, filter_type contains the filter type, and line_buffer contains the data
        let at = j as usize * (row + 1);
        filt[at] = filter_type as u8;
        filt[at + 1..at + 1 + row].copy_from_slice(&line_buffer);
    }
    let zlib = zlib_compress(&filt, options.png_compression_level);
    drop(filt);

    // each tag requires 12 bytes of overhead
    let mut out = Vec::with_capacity(8 + 12 + 13 + 12 + zlib.len() + 12);
    out.extend_from_slice(&sig);
    // (A chunk: its length, its name and its bytes, and the checksum of the last two.)
    let mut chunk = |name: &[u8; 4], bytes: &[u8]| {
        out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        let from = out.len();
        out.extend_from_slice(name);
        out.extend_from_slice(bytes);
        let crc = crc32(&out[from..]);
        out.extend_from_slice(&crc.to_be_bytes());
    };
    let mut header = [0u8; 13];
    header[..4].copy_from_slice(&(x as u32).to_be_bytes());
    header[4..8].copy_from_slice(&(y as u32).to_be_bytes());
    header[8] = 8;
    header[9] = ctype[n as usize] as u8;
    chunk(b"IHDR", &header);
    chunk(b"IDAT", &zlib);
    chunk(b"IEND", &[]);
    Some(out)
}

// ---------------------------------------------------------------------------------------------
// JPEG writer
//
// This is based on Jon Olick's jo_jpeg.cpp:
// public domain Simple, Minimalistic JPEG writer - http://www.jonolick.com/code.html

/// The bits on their way out of the JPEG writer.
struct JpgBits {
    out: Vec<u8>,
    bit_buf: u32,
    bit_cnt: i32,
}

impl JpgBits {
    /// `stbiw__jpg_writeBits`
    fn write_bits(&mut self, bs: [u16; 2]) {
        self.bit_cnt += i32::from(bs[1]);
        self.bit_buf |= u32::from(bs[0]) << (24 - self.bit_cnt);
        while self.bit_cnt >= 8 {
            let c = ((self.bit_buf >> 16) & 255) as u8;
            self.out.push(c);
            if c == 255 {
                self.out.push(0);
            }
            self.bit_buf <<= 8;
            self.bit_cnt -= 8;
        }
    }
}

/// `stbiw__jpg_DCT`, on eight values of `cdu`, `step` apart from `at`.
// (The constants have the original's digits.)
#[allow(clippy::excessive_precision, clippy::approx_constant)]
fn stbiw_jpg_dct(cdu: &mut [f32], at: usize, step: usize) {
    let (d0, d1, d2, d3, d4, d5, d6, d7) = (
        cdu[at],
        cdu[at + step],
        cdu[at + step * 2],
        cdu[at + step * 3],
        cdu[at + step * 4],
        cdu[at + step * 5],
        cdu[at + step * 6],
        cdu[at + step * 7],
    );

    let tmp0 = d0 + d7;
    let tmp7 = d0 - d7;
    let tmp1 = d1 + d6;
    let tmp6 = d1 - d6;
    let tmp2 = d2 + d5;
    let tmp5 = d2 - d5;
    let tmp3 = d3 + d4;
    let tmp4 = d3 - d4;

    // Even part
    let mut tmp10 = tmp0 + tmp3; // phase 2
    let tmp13 = tmp0 - tmp3;
    let mut tmp11 = tmp1 + tmp2;
    let mut tmp12 = tmp1 - tmp2;

    let d0 = tmp10 + tmp11; // phase 3
    let d4 = tmp10 - tmp11;

    let z1 = (tmp12 + tmp13) * 0.707106781; // c4
    let d2 = tmp13 + z1; // phase 5
    let d6 = tmp13 - z1;

    // Odd part
    tmp10 = tmp4 + tmp5; // phase 2
    tmp11 = tmp5 + tmp6;
    tmp12 = tmp6 + tmp7;

    // The rotator is modified from fig 4-8 to avoid extra negations.
    let z5 = (tmp10 - tmp12) * 0.382683433; // c6
    let z2 = tmp10 * 0.541196100 + z5; // c2-c6
    let z4 = tmp12 * 1.306562965 + z5; // c2+c6
    let z3 = tmp11 * 0.707106781; // c4

    let z11 = tmp7 + z3; // phase 5
    let z13 = tmp7 - z3;

    cdu[at + step * 5] = z13 + z2; // phase 6
    cdu[at + step * 3] = z13 - z2;
    cdu[at + step] = z11 + z4;
    cdu[at + step * 7] = z11 - z4;

    cdu[at] = d0;
    cdu[at + step * 2] = d2;
    cdu[at + step * 4] = d4;
    cdu[at + step * 6] = d6;
}

/// `stbiw__jpg_calcBits`
fn stbiw_jpg_calc_bits(mut val: i32) -> [u16; 2] {
    let mut tmp1 = if val < 0 { -val } else { val };
    val = if val < 0 { val - 1 } else { val };
    let mut bits1 = 1u16;
    loop {
        tmp1 >>= 1;
        if tmp1 == 0 {
            break;
        }
        bits1 += 1;
    }
    [(val & ((1 << bits1) - 1)) as u16, bits1]
}

/// `stbiw__jpg_processDU`
fn stbiw_jpg_process_du(
    s: &mut JpgBits,
    cdu: &mut [f32],
    du_stride: usize,
    fdtbl: &[f32; 64],
    dc: i32,
    htdc: &[[u16; 2]; 256],
    htac: &[[u16; 2]; 256],
) -> i32 {
    let eob = htac[0x00];
    let m16zeroes = htac[0xF0];
    let mut du = [0i32; 64];

    // DCT rows
    let mut data_off = 0;
    while data_off < du_stride * 8 {
        stbiw_jpg_dct(cdu, data_off, 1);
        data_off += du_stride;
    }
    // DCT columns
    for data_off in 0..8 {
        stbiw_jpg_dct(cdu, data_off, du_stride);
    }
    // Quantize/descale/zigzag the coefficients
    let mut j = 0;
    for y in 0..8 {
        for x in 0..8 {
            let i = y * du_stride + x;
            let v = cdu[i] * fdtbl[j];
            // DU[stbiw__jpg_ZigZag[j]] = (int)(v < 0 ? ceilf(v - 0.5f) : floorf(v + 0.5f));
            // ceilf() and floorf() are C99, not C89, but I /think/ they're not needed here anyway?
            du[usize::from(STBIW_JPG_ZIGZAG[j])] = (if v < 0.0 { v - 0.5 } else { v + 0.5 }) as i32;
            j += 1;
        }
    }

    // Encode DC
    let diff = du[0] - dc;
    if diff == 0 {
        s.write_bits(htdc[0]);
    } else {
        let bits = stbiw_jpg_calc_bits(diff);
        s.write_bits(htdc[usize::from(bits[1])]);
        s.write_bits(bits);
    }
    // Encode ACs
    let mut end0pos = 63;
    while end0pos > 0 && du[end0pos] == 0 {
        end0pos -= 1;
    }
    // end0pos = first element in reverse order !=0
    if end0pos == 0 {
        s.write_bits(eob);
        return du[0];
    }
    let mut i = 1;
    while i <= end0pos {
        let startpos = i;
        while du[i] == 0 && i <= end0pos {
            i += 1;
        }
        let mut nrzeroes = i - startpos;
        if nrzeroes >= 16 {
            let lng = nrzeroes >> 4;
            for _ in 1..=lng {
                s.write_bits(m16zeroes);
            }
            nrzeroes &= 15;
        }
        let bits = stbiw_jpg_calc_bits(du[i]);
        s.write_bits(htac[(nrzeroes << 4) + usize::from(bits[1])]);
        s.write_bits(bits);
        i += 1;
    }
    if end0pos != 63 {
        s.write_bits(eob);
    }
    du[0]
}

/// The bytes of a `.jpg` file for an image: `stbi_write_jpg_core`. `data` holds `height` rows
/// of `width` pixels from the top, each of `comp` bytes (1 grey, 2 grey and alpha, 3 RGB, 4
/// RGBA; alpha is not written), a row every `stride_in_bytes` bytes (0 for rows that follow
/// one another). `quality` goes from 1 to 100, and 0 is 90; up to 90 the colour is held at
/// half the size each way.
///
/// None when the size is not above zero, `comp` is not 1 to 4, or `data` is too short.
pub fn write_jpg(
    width: i32,
    height: i32,
    comp: i32,
    data: &[u8],
    quality: i32,
    stride_in_bytes: usize,
    options: &WriteOptions,
) -> Option<Vec<u8>> {
    // (The products are single floats, as in the original.)
    #[allow(clippy::excessive_precision)]
    static AASF: [f32; 8] = [
        1.0 * 2.828427125,
        1.387039845 * 2.828427125,
        1.306562965 * 2.828427125,
        1.175875602 * 2.828427125,
        1.0 * 2.828427125,
        0.785694958 * 2.828427125,
        0.541196100 * 2.828427125,
        0.275899379 * 2.828427125,
    ];

    let (mut fdtbl_y, mut fdtbl_uv) = ([0.0f32; 64], [0.0f32; 64]);
    let (mut y_table, mut uv_table) = ([0u8; 64], [0u8; 64]);

    if !stbiw_validate_dimensions(width, height, comp) {
        return None;
    }

    let stride_in_bytes = if stride_in_bytes == 0 {
        comp as usize * width as usize
    } else {
        stride_in_bytes
    };
    if !holds(data, width, height, comp, stride_in_bytes) {
        return None;
    }

    let mut quality = if quality != 0 { quality } else { 90 };
    let subsample = quality <= 90;
    quality = quality.clamp(1, 100);
    quality = if quality < 50 {
        5000 / quality
    } else {
        200 - quality * 2
    };

    for i in 0..64 {
        let yti = (YQT[i] * quality + 50) / 100;
        y_table[usize::from(STBIW_JPG_ZIGZAG[i])] = yti.clamp(1, 255) as u8;
        let uvti = (UVQT[i] * quality + 50) / 100;
        uv_table[usize::from(STBIW_JPG_ZIGZAG[i])] = uvti.clamp(1, 255) as u8;
    }

    let mut k = 0;
    for row in 0..8 {
        for col in 0..8 {
            fdtbl_y[k] = 1.0
                / (f32::from(y_table[usize::from(STBIW_JPG_ZIGZAG[k])]) * AASF[row] * AASF[col]);
            fdtbl_uv[k] = 1.0
                / (f32::from(uv_table[usize::from(STBIW_JPG_ZIGZAG[k])]) * AASF[row] * AASF[col]);
            k += 1;
        }
    }

    let mut s = JpgBits {
        out: Vec::new(),
        bit_buf: 0,
        bit_cnt: 0,
    };

    // Write Headers
    {
        static HEAD0: [u8; 25] = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10, b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0,
            0xFF, 0xDB, 0, 0x84, 0,
        ];
        static HEAD2: [u8; 14] = [0xFF, 0xDA, 0, 0xC, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 0x3F, 0];
        let head1: [u8; 24] = [
            0xFF,
            0xC0,
            0,
            0x11,
            8,
            (height >> 8) as u8,
            height as u8,
            (width >> 8) as u8,
            width as u8,
            3,
            1,
            if subsample { 0x22 } else { 0x11 },
            0,
            2,
            0x11,
            1,
            3,
            0x11,
            1,
            0xFF,
            0xC4,
            0x01,
            0xA2,
            0,
        ];
        s.out.extend_from_slice(&HEAD0);
        s.out.extend_from_slice(&y_table);
        s.out.push(1);
        s.out.extend_from_slice(&uv_table);
        s.out.extend_from_slice(&head1);
        s.out.extend_from_slice(&STD_DC_LUMINANCE_NRCODES[1..]);
        s.out.extend_from_slice(&STD_DC_LUMINANCE_VALUES);
        s.out.push(0x10); // HTYACinfo
        s.out.extend_from_slice(&STD_AC_LUMINANCE_NRCODES[1..]);
        s.out.extend_from_slice(&STD_AC_LUMINANCE_VALUES);
        s.out.push(1); // HTUDCinfo
        s.out.extend_from_slice(&STD_DC_CHROMINANCE_NRCODES[1..]);
        s.out.extend_from_slice(&STD_DC_CHROMINANCE_VALUES);
        s.out.push(0x11); // HTUACinfo
        s.out.extend_from_slice(&STD_AC_CHROMINANCE_NRCODES[1..]);
        s.out.extend_from_slice(&STD_AC_CHROMINANCE_VALUES);
        s.out.extend_from_slice(&HEAD2);
    }

    // Encode 8x8 macroblocks
    {
        static FILL_BITS: [u16; 2] = [0x7F, 7];
        let (mut dcy, mut dcu, mut dcv) = (0, 0, 0);
        // comp == 2 is grey+alpha (alpha is ignored)
        let ofs_g = if comp > 2 { 1 } else { 0 };
        let ofs_b = if comp > 2 { 2 } else { 0 };
        let comp = comp as usize;
        // The Y, U and V of a square of pixels of `size` a side from (x, y), the edges of the
        // image repeated past them.
        let convert =
            |x: i32, y: i32, size: i32, y_out: &mut [f32], u_out: &mut [f32], v_out: &mut [f32]| {
                let mut pos = 0;
                for row in y..y + size {
                    // row >= height => use last input row
                    let clamped_row = if row < height { row } else { height - 1 };
                    let base_p = (if options.flip_vertically_on_write {
                        height - 1 - clamped_row
                    } else {
                        clamped_row
                    }) as usize
                        * stride_in_bytes;
                    for col in x..x + size {
                        // if col >= width => use pixel from last input column
                        let p =
                            base_p + (if col < width { col } else { width - 1 }) as usize * comp;
                        let (r, g, b) = (
                            f32::from(data[p]),
                            f32::from(data[p + ofs_g]),
                            f32::from(data[p + ofs_b]),
                        );
                        y_out[pos] = 0.29900 * r + 0.58700 * g + 0.11400 * b - 128.0;
                        u_out[pos] = -0.16874 * r - 0.33126 * g + 0.50000 * b;
                        v_out[pos] = 0.50000 * r - 0.41869 * g - 0.08131 * b;
                        pos += 1;
                    }
                }
            };
        if subsample {
            let mut y = 0;
            while y < height {
                let mut x = 0;
                while x < width {
                    let (mut yy, mut uu, mut vv) = ([0.0f32; 256], [0.0f32; 256], [0.0f32; 256]);
                    convert(x, y, 16, &mut yy, &mut uu, &mut vv);
                    dcy = stbiw_jpg_process_du(
                        &mut s,
                        &mut yy[0..],
                        16,
                        &fdtbl_y,
                        dcy,
                        &YDC_HT,
                        &YAC_HT,
                    );
                    dcy = stbiw_jpg_process_du(
                        &mut s,
                        &mut yy[8..],
                        16,
                        &fdtbl_y,
                        dcy,
                        &YDC_HT,
                        &YAC_HT,
                    );
                    dcy = stbiw_jpg_process_du(
                        &mut s,
                        &mut yy[128..],
                        16,
                        &fdtbl_y,
                        dcy,
                        &YDC_HT,
                        &YAC_HT,
                    );
                    dcy = stbiw_jpg_process_du(
                        &mut s,
                        &mut yy[136..],
                        16,
                        &fdtbl_y,
                        dcy,
                        &YDC_HT,
                        &YAC_HT,
                    );

                    // subsample U,V
                    {
                        let (mut sub_u, mut sub_v) = ([0.0f32; 64], [0.0f32; 64]);
                        let mut pos = 0;
                        for row in 0..8 {
                            for col in 0..8 {
                                let j = row * 32 + col * 2;
                                sub_u[pos] = (uu[j] + uu[j + 1] + uu[j + 16] + uu[j + 17]) * 0.25;
                                sub_v[pos] = (vv[j] + vv[j + 1] + vv[j + 16] + vv[j + 17]) * 0.25;
                                pos += 1;
                            }
                        }
                        dcu = stbiw_jpg_process_du(
                            &mut s, &mut sub_u, 8, &fdtbl_uv, dcu, &UVDC_HT, &UVAC_HT,
                        );
                        dcv = stbiw_jpg_process_du(
                            &mut s, &mut sub_v, 8, &fdtbl_uv, dcv, &UVDC_HT, &UVAC_HT,
                        );
                    }
                    x += 16;
                }
                y += 16;
            }
        } else {
            let mut y = 0;
            while y < height {
                let mut x = 0;
                while x < width {
                    let (mut yy, mut uu, mut vv) = ([0.0f32; 64], [0.0f32; 64], [0.0f32; 64]);
                    convert(x, y, 8, &mut yy, &mut uu, &mut vv);
                    dcy = stbiw_jpg_process_du(&mut s, &mut yy, 8, &fdtbl_y, dcy, &YDC_HT, &YAC_HT);
                    dcu = stbiw_jpg_process_du(
                        &mut s, &mut uu, 8, &fdtbl_uv, dcu, &UVDC_HT, &UVAC_HT,
                    );
                    dcv = stbiw_jpg_process_du(
                        &mut s, &mut vv, 8, &fdtbl_uv, dcv, &UVDC_HT, &UVAC_HT,
                    );
                    x += 8;
                }
                y += 8;
            }
        }

        // Do the bit alignment of the EOI marker
        s.write_bits(FILL_BITS);
    }

    // EOI
    s.out.push(0xFF);
    s.out.push(0xD9);

    Some(s.out)
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
    fn the_small_zlib_makes_streams_that_decompress() {
        let noise: Vec<u8> = (0..5000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let runs: Vec<u8> = (0..70000u32).map(|i| (i / 300) as u8).collect();
        for data in [&[][..], b"a", b"abc", b"abcabcabcabcabcabc", &noise, &runs] {
            for quality in [0, 5, 8, 30] {
                let packed = zlib_compress(data, quality);
                if data.is_empty() {
                    // (The original writes no block at all for no bytes.)
                    assert_eq!(packed, [0x78, 0x5e, 0, 0, 0, 1]);
                } else {
                    assert_eq!(
                        miniz::uncompress(&packed, data.len()).as_deref(),
                        Some(data)
                    );
                }
            }
        }
        // What does not compress is stored, in blocks of no more than 32,767 bytes.
        let mut state = 7u64;
        let big_noise: Vec<u8> = (0..40000)
            .map(|_| {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                (state >> 40) as u8
            })
            .collect();
        assert_eq!(zlib_compress(&big_noise, 8).len(), 2 + 2 * 5 + 40000 + 4);
        assert!(zlib_compress(&runs, 8).len() < 2000);
    }

    #[test]
    fn a_png_has_its_three_chunks() {
        for comp in 1..=4 {
            let pixels = image(13, 7, comp);
            let file = write_png(&pixels, 0, 13, 7, comp as i32, &WriteOptions::default()).unwrap();
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
            let rows = miniz::uncompress(&file[41..41 + length], 7 * (13 * comp + 1)).unwrap();
            assert_eq!(rows.len(), 7 * (13 * comp + 1));
            // Unfiltered, they are the pixels.
            let plain = write_png(
                &pixels,
                0,
                13,
                7,
                comp as i32,
                &WriteOptions {
                    force_png_filter: 0,
                    ..WriteOptions::default()
                },
            )
            .unwrap();
            let length = u32::from_be_bytes(plain[33..37].try_into().unwrap()) as usize;
            let rows = miniz::uncompress(&plain[41..41 + length], 7 * (13 * comp + 1)).unwrap();
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
        let options = WriteOptions::default();
        assert_eq!(
            write_png(&wide, 20, 5, 3, 3, &options),
            write_png(&pixels, 0, 5, 3, 3, &options)
        );
        let upside_down: Vec<u8> = pixels.chunks(15).rev().flatten().copied().collect();
        let flipped = WriteOptions {
            flip_vertically_on_write: true,
            ..options
        };
        assert_eq!(
            write_png(&upside_down, 0, 5, 3, 3, &flipped),
            write_png(&pixels, 0, 5, 3, 3, &options)
        );
        // No size, too few bytes, too many channels.
        assert!(write_png(&pixels, 0, 0, 3, 3, &options).is_none());
        assert!(write_png(&pixels[..44], 0, 5, 3, 3, &options).is_none());
        assert!(write_png(&pixels, 0, 5, 3, 5, &options).is_none());
    }

    #[test]
    fn a_bmp_is_laid_out_bottom_up() {
        let options = WriteOptions::default();
        let pixels = image(5, 2, 3);
        let file = write_bmp(5, 2, 3, &pixels, 0, &options).unwrap();
        // Rows of 15 bytes padded to 16, behind 54 bytes of headers.
        assert_eq!(file.len(), 54 + 2 * 16);
        assert_eq!(&file[..2], b"BM");
        assert_eq!(u32::from_le_bytes(file[2..6].try_into().unwrap()), 86);
        assert_eq!(u16::from_le_bytes(file[28..30].try_into().unwrap()), 24);
        // The last row first, blue before red.
        assert_eq!(&file[54..57], &[pixels[17], pixels[16], pixels[15]]);
        assert_eq!(file[54 + 15], 0);
        // Grey is three equal values; four channels keep their alpha behind a larger header.
        let grey = write_bmp(2, 1, 1, &[7, 9], 0, &options).unwrap();
        assert_eq!(&grey[54..], &[7, 7, 7, 9, 9, 9, 0, 0]);
        let rgba = write_bmp(1, 1, 4, &[1, 2, 3, 4], 0, &options).unwrap();
        assert_eq!((rgba.len(), &rgba[122..]), (126, &[3u8, 2, 1, 4][..]));
        assert!(write_bmp(0, 1, 3, &pixels, 0, &options).is_none());
        assert!(write_bmp(5, 3, 3, &pixels, 0, &options).is_none());
    }

    #[test]
    fn a_jpg_has_its_markers() {
        let pixels = image(20, 11, 3);
        for (quality, sampling) in [(0, 0x22), (50, 0x22), (95, 0x11)] {
            let file = write_jpg(20, 11, 3, &pixels, quality, 0, &WriteOptions::default()).unwrap();
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
        let small = write_jpg(20, 11, 3, &pixels, 10, 0, &WriteOptions::default()).unwrap();
        let large = write_jpg(20, 11, 3, &pixels, 100, 0, &WriteOptions::default()).unwrap();
        assert!(small.len() < large.len());
        assert!(write_jpg(20, 12, 3, &pixels, 90, 0, &WriteOptions::default()).is_none());
    }
}

/// (The README's examples, compiled and run with the tests.)
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
