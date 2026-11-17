# stb_image_write_nrp

**PNG, BMP and JPEG writers in pure, safe Rust: a native Rust port of stb_image_write.h.**

stb_image_write_nrp is a line-for-line port of three of the writers of Sean Barrett's
[`stb_image_write.h`](https://github.com/nothings/stb) v1.18: PNG, BMP and JPEG. Each gives
the file the original writes, byte for byte, with the same settings.

## Highlights

- **Bit-identical files.** The PNG writer with its own small zlib compressor and its choice
  of filter for each row, the BMP writer, and the baseline JPEG writer after Jon Olick's
  `jo_jpeg.cpp`, its arithmetic in floats as the original's.

- **Zero dependencies.** Standard library only: no C compiler, no build script.

- **No `unsafe` code.** Enforced by `#![forbid(unsafe_code)]`.

## Installation

```toml
[dependencies]
stb_image_write_nrp = "0.1"
```

## Quick start

```rust
use stb_image_write_nrp::{WriteOptions, write_bmp, write_jpg, write_png};

// A gradient, 64 by 32 pixels of RGB.
let (w, h) = (64, 32);
let pixels: Vec<u8> = (0..w * h)
    .flat_map(|i| [(i % w * 4) as u8, (i / w * 8) as u8, 128])
    .collect();
let options = WriteOptions::default();

// Each gives the file's bytes, or none for a size the original refuses.
let png = write_png(&pixels, 3 * w, w as i32, h as i32, 3, &options).unwrap();
assert_eq!(&png[1..4], b"PNG");
let bmp = write_bmp(w as i32, h as i32, 3, &pixels, 3 * w, &options).unwrap();
assert_eq!(&bmp[..2], b"BM");
let jpg = write_jpg(w as i32, h as i32, 3, &pixels, 90, 3 * w, &options).unwrap();
assert_eq!(&jpg[..2], &[0xff, 0xd8]);
```

## What is ported

| Function | Original | What it does |
|---|---|---|
| `write_png` | `stbi_write_png_to_mem` | 8 bits a channel, one to four channels (grey, grey with alpha, RGB, RGBA) |
| `zlib_compress` | `stbi_zlib_compress` | The PNG writer's own zlib compressor |
| `write_bmp` | `stbi_write_bmp_to_func` | 24 bits a pixel, or 32 with alpha when there are four channels |
| `write_jpg` | `stbi_write_jpg_to_func` | Baseline JPEG at a quality from 1 to 100 |

The three settings the original keeps in globals (`stbi_write_png_compression_level`,
`stbi_write_force_png_filter`, `stbi_flip_vertically_on_write`) are a `WriteOptions` handed to
each call. Not ported: the `.tga` and `.hdr` writers, and writing through a callback or
straight to a file.

## Differences from the original

The code is translated from C to Rust. Each writer gives back the file's bytes, or none where
the original refuses the image, rather than writing them through a callback. The settings are
handed to each call rather than kept in globals. What they compute is kept: the same file,
byte for byte, for the same image and settings.

## Verification

`compare/compare.sh` builds `compare/dump_stb_image_write.c` against `stb_image_write.h` from
a checkout of stb, and `examples/dump.rs` against the port; it runs both over the same made-up
images and compares what they print byte for byte:

```sh
compare/compare.sh <checkout of stb> [count] [seeds] [dumps]
```

| Dump | What it covers | Last full run |
|---|---|---|
| `stb_image_write` | Made-up images of one to four channels, of seven kinds and many sizes, some with rows further apart than they are wide, written as `.png` (with made-up compression levels and filters), `.bmp` and `.jpg` (with made-up qualities), some upside down; and made-up data through the zlib compressor | 9,000 images (32 million pixels), each as three identical files; the C built by gcc and clang at several optimisation levels |

`cargo test` needs no C compiler: the tests check the port against hashes recorded from
`stb_image_write.h` (`tests/data`).

## Licence

As stb is: in the public domain (the Unlicense) or MIT, whichever you prefer
([LICENSE](LICENSE), which keeps stb's licence).
