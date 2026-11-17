//! The port of stb_image_write.h against results recorded from the original, so that a change
//! shows without a C compiler at hand.
//!
//! `tests/data/stb_image_write_expected.txt` is what `compare/dump_stb_image_write.c`, built
//! against stb (see `compare/compare.sh`), prints for `stb_image_write 5 400` and
//! `stb_image_write 12 400`: a hash of the `.png`, `.bmp` and `.jpg` files made-up images are
//! written as, with made-up settings, and of what made-up data comes to in its zlib
//! compressor. `compare/compare.sh` is the full comparison.

mod made_up;

#[test]
fn writes_the_files_stb_image_write_does() {
    let printed = [
        made_up::stb_image_write(5, 400),
        made_up::stb_image_write(12, 400),
    ]
    .concat();
    let expected = include_str!("data/stb_image_write_expected.txt");
    for (line, (ours, theirs)) in printed.lines().zip(expected.lines()).enumerate() {
        assert_eq!(ours, theirs, "line {}", line + 1);
    }
    assert_eq!(printed.lines().count(), expected.lines().count());
}
