# Recorded results

`stb_image_write_expected.txt` is what stb_image_write.h makes of made-up images, as
`compare/dump_stb_image_write.c` prints it; `tests/golden.rs` checks that the port prints the
same. To make it again, run the comparison once (it builds the original's side), then that by
itself:

    compare/compare.sh <checkout of stb> 3000 1
    ref="${TMPDIR:-/tmp}/stb_image_write_nrp-compare/dump-stb-image-write"
    { "$ref" stb_image_write 5 400; "$ref" stb_image_write 12 400; } > tests/data/stb_image_write_expected.txt

No picture or file from anywhere is in this repository: every file and stream the tests and
the comparison use is made up from a seed.
