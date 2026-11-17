# Recorded results

`stb_image_write_expected.txt` holds the hashes `compare/dump_stb_image_write.c` prints when
built against `stb_image_write.h` v1.18, for synthetic images. `tests/golden.rs` checks that
stb_image_write_nrp prints the same.

To record them again, run the comparison once, which builds the original's side, then run
that program directly:

    compare/compare.sh <checkout of stb> 3000 1
    ref="${TMPDIR:-/tmp}/stb_image_write_nrp-compare/dump-stb-image-write"
    { "$ref" stb_image_write 5 400; "$ref" stb_image_write 12 400; } > tests/data/stb_image_write_expected.txt

All test data is generated from seeds; no external files are used.
