# stb_image_write_nrp

**PNG, BMP and JPEG writers in pure, safe Rust, bit-identical to stb_image_write.h.**

**Overview** · [Guide](docs/guide.md) · [Verification](docs/verification.md) · [Performance](docs/performance.md)

---

stb_image_write_nrp is a native Rust port of three of the writers of Sean Barrett's
[`stb_image_write.h`](https://github.com/nothings/stb) v1.18: PNG, BMP and JPEG. Each writes
the same file as the original, byte for byte, for the same image and settings.

## Highlights

- **Bit-identical files.** The PNG writer with its per-row filter choice and its own small
  zlib compressor, the BMP writer, and the baseline JPEG writer after Jon Olick's
  `jo_jpeg.cpp`, with its floating-point arithmetic kept in the original's order.

- **Checked once, written infallibly.** An `Image` validates its size, channels, stride and
  pixel buffer when it is made, so the writers return their bytes directly.

- **Zero dependencies.** Standard library only: no C compiler, no build script.

- **No `unsafe` code.** Enforced by `#![forbid(unsafe_code)]`.

## Installation

```toml
[dependencies]
stb_image_write_nrp = "0.2"
```

## Quick start

```rust
use stb_image_write_nrp::{Channels, Image, WriteOptions, write_bmp, write_jpg, write_png};

// A gradient, 64 by 32 pixels of RGB.
let (width, height) = (64, 32);
let pixels: Vec<u8> = (0..width * height)
    .flat_map(|i| [(i % width * 4) as u8, (i / width * 8) as u8, 128])
    .collect();
let image = Image::new(&pixels, width, height, Channels::Rgb).unwrap();
let options = WriteOptions::default();

let png = write_png(&image, &options);
assert_eq!(&png[1..4], b"PNG");
let bmp = write_bmp(&image, &options);
assert_eq!(&bmp[..2], b"BM");
let jpg = write_jpg(&image, 90, &options);
assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
```

## Scope

PNG (8 bits a channel), BMP (24 bits a pixel, or 32 with alpha) and baseline JPEG, each to
bytes in memory. The original's TGA and HDR writers, and its writing through a callback or to
a file, are not ported.

## Licence

As stb is: in the public domain (the Unlicense) or MIT, whichever you prefer; see
[`LICENSE`](LICENSE), which keeps stb's licence.

Bug reports and contributions are welcome, above all any image that stb_image_write_nrp
writes differently from `stb_image_write.h`.
