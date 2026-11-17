#!/bin/sh
# Compares stb_image_write_nrp with stb_image_write.h of stb (https://github.com/nothings/stb).
#
#   compare/compare.sh <checkout of stb> [count] [seeds] [dumps]
#
# compare/dump_stb_image_write.c is built against its stb_image_write.h, without contraction of a
# multiply and an add (its JPEG writer works in floats), and examples/dump.rs from the port; both
# run the same synthetic images for each seed and what they print is compared byte for byte.
# Dump: stb_image_write, with a tenth of [count] (200,000 by default) as the number of synthetic
# images, each written as .png, .bmp and .jpg.
set -u
if [ $# -lt 1 ]; then
    echo "usage: $0 <checkout of stb> [count] [seeds] [dumps]" >&2
    exit 2
fi
here=$(cd "$(dirname "$0")/.." && pwd)
source=$1
blocks=${2:-200000}
seeds=${3:-3}
dumps=${4:-stb_image_write}
tmp=${TMPDIR:-/tmp}/stb_image_write_nrp-compare
mkdir -p "$tmp" || exit 2
if ! [ -f "$source/stb_image_write.h" ]; then
    echo "$source does not look like checkout of stb" >&2
    exit 2
fi

(cd "$here" && cargo build --release -q --ignore-rust-version --example dump) || exit 2
rust=$(cd "$here" && cargo metadata --format-version 1 --no-deps 2>/dev/null | sed 's/.*"target_directory":"\([^"]*\)".*/\1/')/release/examples/dump

${CC:-cc} -O2 -ffp-contract=off -I "$source" "$here/compare/dump_stb_image_write.c" -lm -o "$tmp/dump-stb-image-write" || exit 2

bad=0
for what in $dumps; do
    extra=
    case $what in
    stb_image_write)
        reference=$tmp/dump-stb-image-write
        count=$((blocks / 10))
        ;;
    *)
        echo "unknown dump $what" >&2
        exit 2
        ;;
    esac
    seed=1
    while [ "$seed" -le "$seeds" ]; do
        # shellcheck disable=SC2086
        "$reference" "$what" "$seed" "$count" $extra > "$tmp/reference.txt" 2> /dev/null
        # shellcheck disable=SC2086
        "$rust" "$what" "$seed" "$count" $extra > "$tmp/rust.txt"
        if [ ! -s "$tmp/reference.txt" ]; then
            echo "$what, seed $seed: NOT COMPARED, the original's side printed nothing"
            bad=$((bad + 1))
        elif cmp -s "$tmp/reference.txt" "$tmp/rust.txt"; then
            echo "$what, seed $seed: the same ($(tail -1 "$tmp/rust.txt"))"
        else
            echo "$what, seed $seed: DIFFERENT"
            bad=$((bad + 1))
        fi
        seed=$((seed + 1))
    done
done
[ "$bad" -eq 0 ]
