//! The Rust half of the comparison with `stb_image_write.h`: runs the port over synthetic data and
//! prints a running hash of what comes out, as `compare/dump_stb_image_write.c` does with the
//! original. `compare/compare.sh` compares the two.
//!
//!   dump `stb_image_write` <seed> <count>   synthetic images of one to four channels, of seven
//!                                         kinds and many sizes, some with rows further apart
//!                                         than they are wide, written as .png (with synthetic
//!                                         compression levels and filters), as .bmp and as .jpg
//!                                         (with random qualities), some upside down; and for
//!                                         every fourth, synthetic data through the file's own
//!                                         zlib compressor

#[path = "../tests/made_up/mod.rs"]
mod made_up;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(what), Some(seed), Some(count)) = (
        args.get(1),
        args.get(2).and_then(|s| s.parse::<u64>().ok()),
        args.get(3).and_then(|s| s.parse::<u32>().ok()),
    ) else {
        eprintln!("usage: dump <what> <seed> <count>   (see the top of dump.rs)");
        std::process::exit(2);
    };
    let _ = seed;
    if what.as_str() == "stb_image_write" {
        print!("{}", made_up::stb_image_write(seed, count));
    } else {
        eprintln!("unknown dump {what}");
        std::process::exit(2);
    }
}
