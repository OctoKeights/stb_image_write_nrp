# Performance

[Overview](../README.md) · [Guide](guide.md) · [Verification](verification.md) · **Performance**

---

`examples/bench.rs` writes a synthetic 1024 by 768 image, photograph-like (smooth gradients
with a little noise, and some flat areas), in each format, and reports the best of nine runs:

```sh
cargo run --release --example bench
```

## Throughput

Measured on one core of an Intel Xeon at 2.1 GHz, built for generic x86-64, in megapixels
per second:

| Format | RGB | RGBA |
|---|---|---|
| PNG | 11.5 | 11.2 |
| BMP | 719 | 684 |
| JPEG, quality 90 | 71.5 | 71.2 |
| JPEG, quality 95 | 43.8 | 43.9 |

PNG is the slowest by far: every row is filtered five ways, and the compressor examines up to
twice `png_compression_level` earlier positions for every byte. Above quality 90, JPEG keeps
the chroma at full resolution, which doubles the blocks to encode.

## Compared with 0.1

Version 0.2 rewrote the crate as idiomatic Rust. Measured alternately with 0.1 on the same
machine:

| Format | RGB | RGBA |
|---|---|---|
| PNG | 101% faster | 117% faster |
| BMP | 44% faster | 74% faster |
| JPEG, quality 90 | 2% faster | 6% faster |
| JPEG, quality 95 | 5% faster | 8% faster |

PNG gains most from comparing candidate matches eight bytes at a time and from keeping the
best filtered row rather than filtering it a sixth time; BMP from converting whole rows at a
time; JPEG from a branch-free rounding of its coefficients.
