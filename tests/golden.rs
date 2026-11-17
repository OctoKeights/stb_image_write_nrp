//! stb_image_write_nrp against hashes recorded from `stb_image_write.h`, so that any change in
//! output shows without a C compiler.
//!
//! `tests/data/stb_image_write_expected.txt` is what `compare/dump_stb_image_write.c`, built
//! against stb_image_write.h v1.18, prints for `stb_image_write 5 400` and
//! `stb_image_write 12 400`: hashes of the PNG, BMP and JPEG files of synthetic images, with
//! random settings, and of synthetic data through the PNG writer's zlib compressor.
//! `compare/compare.sh` runs the full comparison.

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
