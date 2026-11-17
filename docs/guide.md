# Guide

[Overview](../README.md) · **Guide** · [Verification](verification.md) · [Performance](performance.md)

---

- [Images](#images)
- [Options](#options)
- [PNG](#png)
- [BMP](#bmp)
- [JPEG](#jpeg)
- [Correspondence with stb_image_write.h](#correspondence-with-stb_image_writeh)
- [Differences from stb_image_write.h](#differences-from-stb_image_writeh)

## Images

An `Image` borrows the pixels to write: `height` rows of `width` pixels from the top, 8 bits
a channel, with one to four `Channels`:

| Channels | Bytes a pixel |
|---|---|
| `Channels::Grey` | 1 |
| `Channels::GreyAlpha` | 2: grey, then alpha |
| `Channels::Rgb` | 3 |
| `Channels::Rgba` | 4 |

`Image::new` takes rows that follow one another; `Image::with_stride` takes rows that start
further apart, as in a region of a larger buffer, and does not read the bytes between them.
Both check the image when it is made, and return an `ImageError` if the width or height is
zero or above `MAX_DIMENSION`, if the stride is shorter than a row, or if the pixels end
before the last row:

```rust
use stb_image_write_nrp::{Channels, Image, ImageError};

let buffer = vec![0u8; 100 * 50 * 4];
// The left half of a 100 by 50 RGBA buffer.
let left = Image::with_stride(&buffer, 50, 50, Channels::Rgba, 100 * 4).unwrap();
assert_eq!(left.width(), 50);

assert_eq!(
    Image::new(&buffer[..10], 100, 50, Channels::Rgba).unwrap_err(),
    ImageError::NotEnoughPixels
);
```

Once made, an image can be written in any format without further checks.

## Options

`WriteOptions` holds the three settings the original keeps in globals:

| Field | Default | Effect |
|---|---|---|
| `png_compression_level` | 8 | How hard the PNG compressor searches for repeats; below 5 counts as 5 |
| `png_filter` | `None` | The filter for every PNG row, or `None` to choose the best for each row |
| `flip_vertically` | `false` | Write the image upside down, in every format |

## PNG

`write_png` filters each row, compresses the rows with `zlib_compress`, and writes the IHDR,
IDAT and IEND chunks. With `png_filter` left at `None`, each row is filtered all five ways and
the one whose bytes, taken as signed values, sum to the least is kept:

```rust
use stb_image_write_nrp::{Channels, Image, PngFilter, WriteOptions, write_png};

let pixels = vec![200u8; 16 * 16];
let image = Image::new(&pixels, 16, 16, Channels::Grey).unwrap();
let options = WriteOptions {
    png_filter: Some(PngFilter::Paeth),
    ..WriteOptions::default()
};
let png = write_png(&image, &options);
```

`zlib_compress` is the original's own small compressor, available on its own: one block in
DEFLATE's fixed Huffman codes, with matches found through a hash table that keeps between
`level` and twice `level` positions for each hash; or the data stored as it is, when that is
smaller. It is fast and simple rather than compact. For better compression of other data,
[miniz_nrp](https://github.com/OctoKeights/miniz_nrp) offers full zlib compression.

## BMP

`write_bmp` writes rows from the bottom, as BMP stores them, each padded to a multiple of
four bytes. Grey is written as three equal values, and the alpha of `Channels::GreyAlpha` is
dropped. `Channels::Rgba` is written as 32 bits a pixel with an alpha mask in a version 4
header, which most readers honour.

## JPEG

`write_jpg` writes a baseline JPEG with the standard quantisation tables scaled by `quality`,
from 1 (smallest) to 100 (best). A quality of 0 means 90, and values above 100 count as 100.
Up to 90, the chroma is stored at half the resolution each way; above 90, at full
resolution. Grey images are written as three components, and alpha is ignored.

```rust
use stb_image_write_nrp::{Channels, Image, WriteOptions, write_jpg};

let pixels: Vec<u8> = (0..32 * 32 * 3).map(|i| (i % 251) as u8).collect();
let image = Image::new(&pixels, 32, 32, Channels::Rgb).unwrap();
let small = write_jpg(&image, 30, &WriteOptions::default());
let large = write_jpg(&image, 95, &WriteOptions::default());
assert!(small.len() < large.len());
```

## Correspondence with stb_image_write.h

| stb_image_write.h | stb_image_write_nrp |
|---|---|
| `stbi_write_png_to_mem(pixels, stride, x, y, n, &len)` | `write_png(&Image::with_stride(pixels, x, y, channels, stride)?, &options)` |
| `stbi_write_bmp_to_func(func, ctx, x, y, comp, data)` | `write_bmp(&image, &options)` |
| `stbi_write_jpg_to_func(func, ctx, x, y, comp, data, quality)` | `write_jpg(&image, quality, &options)` |
| `stbi_zlib_compress(data, len, &out_len, quality)` | `zlib_compress(data, level)` |
| `stbi_write_png_compression_level` | `WriteOptions::png_compression_level` |
| `stbi_write_force_png_filter` | `WriteOptions::png_filter` |
| `stbi_flip_vertically_on_write` | `WriteOptions::flip_vertically` |

## Differences from stb_image_write.h

- **Settings are passed to each call**, not kept in globals, so different threads can write
  with different settings.

- **Images are validated up front.** The original writes nothing (or, for BMP, a header with
  no pixels) for an invalid size, and trusts its caller about the buffer's length; here an
  `Image` cannot be made for either.

- **Files are returned as bytes**, not written through a callback or to a path.
