//! The PNG writer's own zlib compressor (`stbi_zlib_compress`).

/// The hash table's size: positions are filed under one of this many hashes of their first
/// three bytes.
const HASH_SIZE: usize = 16384;
const WINDOW: i64 = 32768;
const MAX_MATCH: usize = 258;

/// The first length of each length code, and one past the last.
const LENGTH_BASE: [u16; 30] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258, 259,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
/// The first distance of each distance code, and one past the last.
const DIST_BASE: [u16; 31] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577, 32768,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

/// Compresses `data` to a zlib stream, the simple way of the original's PNG writer
/// (`stbi_zlib_compress`): one block in DEFLATE's fixed codes, or the bytes stored as they are
/// when that would be larger.
///
/// `level` is how many earlier positions are kept for each hash of three bytes, between half
/// as many and twice as many: more finds more repeats, more slowly. Below 5 counts as 5.
///
/// ```
/// use stb_image_write_nrp::zlib_compress;
///
/// let stream = zlib_compress(&b"abcabcabcabc".repeat(100), 8);
/// assert_eq!(&stream[..2], &[0x78, 0x5E]);
/// ```
pub fn zlib_compress(data: &[u8], level: u32) -> Vec<u8> {
    let quality = level.max(5) as usize;
    let len = data.len();
    let mut z = BitWriter::default();
    // For each hash, the latest positions with it, oldest first.
    let mut hash_table: Vec<Vec<u32>> = vec![Vec::new(); HASH_SIZE];

    z.out.extend_from_slice(&[0x78, 0x5E]); // a 32 KiB window; FLEVEL 1
    z.write(1, 1); // the final block,
    z.write(1, 2); // in the fixed codes

    let mut i = 0;
    while i + 3 < len {
        let h = hash(&data[i..]);
        let mut best = 3;
        let mut best_at = None;
        for &place in &hash_table[h] {
            let place = place as usize;
            if place as i64 > i as i64 - WINDOW {
                let d = match_len(&data[place..], &data[i..]);
                if d >= best {
                    best = d;
                    best_at = Some(place);
                }
            }
        }
        // A full list loses its older half.
        let list = &mut hash_table[h];
        if list.len() == 2 * quality {
            list.drain(..quality);
        }
        list.push(i as u32);

        if best_at.is_some() {
            // Lazy matching: if the next position has a longer match, this byte is a literal.
            for &place in &hash_table[hash(&data[i + 1..])] {
                let place = place as usize;
                if place as i64 > i as i64 - (WINDOW - 1)
                    && match_len(&data[place..], &data[i + 1..]) > best
                {
                    best_at = None;
                    break;
                }
            }
        }

        match best_at {
            Some(place) => {
                let dist = i - place;
                debug_assert!(dist < WINDOW as usize && best <= MAX_MATCH);
                let code = LENGTH_BASE[1..]
                    .iter()
                    .position(|&b| best < usize::from(b))
                    .unwrap();
                z.write_fixed(257 + code as u32);
                if LENGTH_EXTRA[code] != 0 {
                    z.write(
                        (best - usize::from(LENGTH_BASE[code])) as u32,
                        u32::from(LENGTH_EXTRA[code]),
                    );
                }
                let code = DIST_BASE[1..]
                    .iter()
                    .position(|&b| dist < usize::from(b))
                    .unwrap();
                z.write(reverse(code as u32, 5), 5);
                if DIST_EXTRA[code] != 0 {
                    z.write(
                        (dist - usize::from(DIST_BASE[code])) as u32,
                        u32::from(DIST_EXTRA[code]),
                    );
                }
                i += best;
            }
            None => {
                z.write_literal(data[i]);
                i += 1;
            }
        }
    }
    for &byte in &data[i.min(len)..] {
        z.write_literal(byte);
    }
    z.write_fixed(256); // the end of the block
    if z.count != 0 {
        z.write(0, 8 - z.count);
    }

    let mut out = z.out;
    // Stored, if that is smaller: blocks of up to 32,767 bytes, each with five bytes of header.
    if out.len() > len + 2 + len.div_ceil(32767) * 5 {
        out.truncate(2);
        let mut blocks = data.chunks(32767).peekable();
        while let Some(block) = blocks.next() {
            let block_len = block.len() as u16;
            out.push(u8::from(blocks.peek().is_none()));
            out.extend_from_slice(&block_len.to_le_bytes());
            out.extend_from_slice(&(!block_len).to_le_bytes());
            out.extend_from_slice(block);
        }
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// The hash of the three bytes at the start of `data` (`stbiw__zhash`).
#[inline]
fn hash(data: &[u8]) -> usize {
    let mut hash = u32::from(data[0]) | u32::from(data[1]) << 8 | u32::from(data[2]) << 16;
    hash ^= hash << 3;
    hash = hash.wrapping_add(hash >> 5);
    hash ^= hash << 4;
    hash = hash.wrapping_add(hash >> 17);
    hash ^= hash << 25;
    hash = hash.wrapping_add(hash >> 6);
    hash as usize & (HASH_SIZE - 1)
}

/// How many bytes, up to the longest match, `earlier` and `here` have in common at their start
/// (`stbiw__zlib_countm`). `here` ends with the data; `earlier` is a longer slice of it.
#[inline]
fn match_len(earlier: &[u8], here: &[u8]) -> usize {
    let limit = here.len().min(MAX_MATCH);
    let (a, b) = (&earlier[..limit], &here[..limit]);
    let mut len = 0;
    for (x, y) in a.as_chunks::<8>().0.iter().zip(b.as_chunks::<8>().0) {
        let (x, y) = (u64::from_le_bytes(*x), u64::from_le_bytes(*y));
        if x != y {
            return len + (x ^ y).trailing_zeros() as usize / 8;
        }
        len += 8;
    }
    len + a[len..]
        .iter()
        .zip(&b[len..])
        .take_while(|(x, y)| x == y)
        .count()
}

/// The low `bits` bits of `code`, in reverse order.
#[inline]
fn reverse(code: u32, bits: u32) -> u32 {
    code.reverse_bits() >> (32 - bits)
}

/// The Adler-32 checksum of `data`.
fn adler32(data: &[u8]) -> u32 {
    let (mut s1, mut s2) = (1u32, 0u32);
    let first = data.len() % 5552;
    let (head, rest) = data.split_at(first);
    for chunk in std::iter::once(head).chain(rest.chunks(5552)) {
        for &byte in chunk {
            s1 += u32::from(byte);
            s2 += s1;
        }
        s1 %= 65521;
        s2 %= 65521;
    }
    s2 << 16 | s1
}

/// The stream, written a bit at a time, least significant first.
#[derive(Default)]
struct BitWriter {
    out: Vec<u8>,
    buffer: u32,
    count: u32,
}

impl BitWriter {
    #[inline]
    fn write(&mut self, bits: u32, len: u32) {
        self.buffer |= bits << self.count;
        self.count += len;
        while self.count >= 8 {
            self.out.push(self.buffer as u8);
            self.buffer >>= 8;
            self.count -= 8;
        }
    }

    /// Writes `sym` in DEFLATE's fixed literal and length code (`stbiw__zlib_huff`).
    #[inline]
    fn write_fixed(&mut self, sym: u32) {
        let (code, len) = match sym {
            0..=143 => (0x30 + sym, 8),
            144..=255 => (0x190 + sym - 144, 9),
            256..=279 => (sym - 256, 7),
            _ => (0xC0 + sym - 280, 8),
        };
        self.write(reverse(code, len), len);
    }

    #[inline]
    fn write_literal(&mut self, byte: u8) {
        self.write_fixed(u32::from(byte));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use miniz_nrp as miniz;

    #[test]
    fn the_small_zlib_makes_streams_that_decompress() {
        let noise: Vec<u8> = (0..5000u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 13) as u8)
            .collect();
        let runs: Vec<u8> = (0..70000u32).map(|i| (i / 300) as u8).collect();
        for data in [&[][..], b"a", b"abc", b"abcabcabcabcabcabc", &noise, &runs] {
            for level in [0, 5, 8, 30] {
                let packed = zlib_compress(data, level);
                if data.is_empty() {
                    // (The original writes no block at all for no bytes.)
                    assert_eq!(packed, [0x78, 0x5e, 0, 0, 0, 1]);
                } else {
                    assert_eq!(miniz::decompress(&packed, data.len()).as_deref(), Ok(data));
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
}
