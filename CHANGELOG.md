# Changelog

## 0.2.1

- **Stricter lints, enforced in CI.** `Cargo.toml` turns on clippy's pedantic group for every
  target, and forbids `unsafe` code in the tests, examples and benchmarks as well as the
  library. `Image::new` and `Image::with_stride` document their errors.

- **No panic paths in the library.** The zlib writer finds its length and distance codes
  with `partition_point` instead of a search that was unwrapped, and the PNG writer no
  longer unwraps a row's first byte.

- **Tidier internals.** The JPEG writer's colour conversion returns its Y, Cb and Cr planes
  as one value instead of filling three buffers passed in, and options small enough to copy
  are passed by value.

- Output is unchanged, checked against `stb_image_write.h` built with gcc and with clang on
  18,000 images (63 million pixels), and so is speed.

## 0.2.0

- **Breaking: an idiomatic API.** The pixels to write are an `Image`, made with
  `Image::new` or `Image::with_stride` from a buffer, a size and a `Channels`, and checked
  once, returning an `ImageError`; the writers then take it and return the file directly:
  `write_png(&image, &options)`, `write_bmp(&image, &options)` and
  `write_jpg(&image, quality, &options)`, in place of positional sizes, a channel count and
  an `Option`. `WriteOptions::png_filter` is an `Option<PngFilter>` in place of
  `force_png_filter`'s numbers, `png_compression_level` and `zlib_compress`'s level are
  unsigned, the JPEG quality is a `u8`, and `flip_vertically_on_write` is
  `flip_vertically`.

- **Faster.** PNG is 101% to 117% faster, BMP 44% to 74%, and JPEG 2% to 8% (see
  [Performance](docs/performance.md)). Output is unchanged, checked against
  `stb_image_write.h` on 18,000 images, built with gcc and with clang.

- **Rewritten as idiomatic Rust.** The code no longer mirrors the original's C line by line.

- **Documentation.** Every public item is documented, the README is a short overview, and
  the detail is in [Guide](docs/guide.md), [Verification](docs/verification.md) and
  [Performance](docs/performance.md).

## 0.1.0

- First release: stb_image_write.h's PNG, BMP and JPEG writers, moved out of
  [rustbcx](https://github.com/OctoKeights/rustbcx), where it was
  `rustbcx::stb_image_write`.
