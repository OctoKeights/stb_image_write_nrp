# Verification

[Overview](../README.md) · [Guide](guide.md) · **Verification** · [Performance](performance.md)

---

stb_image_write_nrp is checked against `stb_image_write.h` itself: both are run over the same
synthetic images and their output compared byte for byte.

## The comparison

`compare/compare.sh` builds `compare/dump_stb_image_write.c` against `stb_image_write.h` from
a checkout of stb, with floating-point contraction disabled so that the C compiler does not
fuse the JPEG writer's multiplies and adds, and `examples/dump.rs` against
stb_image_write_nrp; it runs both and compares what they print:

```sh
compare/compare.sh <checkout of stb> [count] [seeds]
CC=clang compare/compare.sh <checkout of stb> [count] [seeds]
```

Both programs generate the same images from each seed and print a running hash of every
file they write:

- **Images** of one to four channels, of seven kinds (noise, flat colour, gradients,
  squares, noisy gradients, scattered colours and waves) and many sizes, some with rows
  further apart than they are wide.

- **Files**: each image as PNG, with random compression levels and filters, as BMP, and as
  JPEG, with random qualities, some upside down.

- **zlib**: synthetic data of eight kinds through the PNG writer's compressor, at random
  levels.

## Results

| Images | Pixels | Compiler | Result |
|---|---|---|---|
| 18,000 | 63 million | gcc | Every PNG, BMP and JPEG file identical |
| 18,000 | 63 million | clang | Every PNG, BMP and JPEG file identical |

## Tests

`cargo test` needs no C compiler. `tests/golden.rs` checks stb_image_write_nrp against hashes
recorded from `stb_image_write.h` (`tests/data/stb_image_write_expected.txt`, 800 images),
unit tests check the files' structure and that the PNG and zlib streams decompress, and the
examples in the README and the Guide are run as tests. `tests/data/README.md` explains how to
record the hashes again.
