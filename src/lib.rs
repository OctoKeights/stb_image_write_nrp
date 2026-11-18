//! PNG, BMP and JPEG writers in pure, safe Rust: a native port of three of the writers of Sean
//! Barrett's [`stb_image_write.h`](https://github.com/nothings/stb) v1.18. Each writes the
//! same file as the original, byte for byte, with the same settings.
//!
//! An [`Image`] describes the pixels to write: 8 bits a channel, one to four [`Channels`], rows
//! from the top, optionally further apart than they are wide. It is checked once, when it is
//! made, so the writers cannot fail:
//!
//! - [`write_png`]: PNG, with a filter chosen for each row and the original's own small zlib
//!   compressor ([`zlib_compress`]).
//!
//! - [`write_bmp`]: BMP, 24 bits a pixel, or 32 with alpha when there are four channels.
//!
//! - [`write_jpg`]: baseline JPEG, after Jon Olick's `jo_jpeg.cpp`.
//!
//! ```
//! use stb_image_write_nrp::{Channels, Image, WriteOptions, write_png};
//!
//! let pixels = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
//! let image = Image::new(&pixels, 2, 2, Channels::Rgb).unwrap();
//! let png = write_png(&image, &WriteOptions::default());
//! assert_eq!(&png[1..4], b"PNG");
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod bmp;
mod jpeg;
mod png;
mod tables;
mod zlib;

pub use bmp::write_bmp;
pub use jpeg::write_jpg;
pub use png::write_png;
pub use zlib::zlib_compress;

use core::fmt;

/// The channels of each pixel, one byte each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channels {
    /// One channel: grey.
    Grey = 1,
    /// Two channels: grey, then alpha.
    GreyAlpha = 2,
    /// Three channels: red, green and blue.
    Rgb = 3,
    /// Four channels: red, green, blue and alpha.
    Rgba = 4,
}

impl Channels {
    /// The channels for a count of 1 to 4.
    #[must_use]
    pub const fn from_count(count: u8) -> Option<Channels> {
        match count {
            1 => Some(Channels::Grey),
            2 => Some(Channels::GreyAlpha),
            3 => Some(Channels::Rgb),
            4 => Some(Channels::Rgba),
            _ => None,
        }
    }

    /// The number of channels, which is also the bytes in a pixel.
    #[must_use]
    pub const fn count(self) -> usize {
        self as usize
    }

    /// Whether the last channel is alpha.
    #[must_use]
    pub const fn has_alpha(self) -> bool {
        matches!(self, Channels::GreyAlpha | Channels::Rgba)
    }
}

/// The pixels of an image to write: `height` rows of `width` pixels, from the top, each row
/// `stride` bytes after the one before.
#[derive(Clone, Copy, Debug)]
pub struct Image<'a> {
    pixels: &'a [u8],
    width: u32,
    height: u32,
    channels: Channels,
    stride: usize,
}

/// The largest width or height the writers accept (`INT_MAX / 4`, as the original has it).
pub const MAX_DIMENSION: u32 = i32::MAX as u32 / 4;

impl<'a> Image<'a> {
    /// An image whose rows follow one another without gaps.
    ///
    /// # Errors
    ///
    /// As [`Image::with_stride`] gives them.
    pub fn new(
        pixels: &'a [u8],
        width: u32,
        height: u32,
        channels: Channels,
    ) -> Result<Image<'a>, ImageError> {
        Image::with_stride(
            pixels,
            width,
            height,
            channels,
            width as usize * channels.count(),
        )
    }

    /// An image whose rows start `stride` bytes apart, which may be more than the bytes of a
    /// row; whatever lies between rows is not read.
    ///
    /// # Errors
    ///
    /// [`ImageError::Empty`] if the width or the height is zero, [`ImageError::TooLarge`] if
    /// either is above [`MAX_DIMENSION`], [`ImageError::StrideTooShort`] if `stride` is less
    /// than the bytes of a row, and [`ImageError::NotEnoughPixels`] if `pixels` ends before
    /// the last row does.
    pub fn with_stride(
        pixels: &'a [u8],
        width: u32,
        height: u32,
        channels: Channels,
        stride: usize,
    ) -> Result<Image<'a>, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::Empty);
        }
        if width > MAX_DIMENSION || height > MAX_DIMENSION {
            return Err(ImageError::TooLarge);
        }
        let row = width as usize * channels.count();
        if stride < row {
            return Err(ImageError::StrideTooShort);
        }
        let needed = (height as usize - 1)
            .checked_mul(stride)
            .and_then(|before| before.checked_add(row));
        if needed.is_none_or(|needed| pixels.len() < needed) {
            return Err(ImageError::NotEnoughPixels);
        }
        Ok(Image {
            pixels,
            width,
            height,
            channels,
            stride,
        })
    }

    /// The width in pixels.
    #[must_use]
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height in pixels.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The channels of each pixel.
    #[must_use]
    pub fn channels(&self) -> Channels {
        self.channels
    }

    /// The bytes from the start of one row to the start of the next.
    #[must_use]
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// The bytes of row `y`, counted from the top.
    fn row(&self, y: usize) -> &'a [u8] {
        let start = y * self.stride;
        &self.pixels[start..start + self.width as usize * self.channels.count()]
    }

    /// Row `y` of the file, which is row `y` of the image, or counted from the bottom when
    /// `flip`.
    fn file_row(&self, y: usize, flip: bool) -> &'a [u8] {
        self.row(if flip {
            self.height as usize - 1 - y
        } else {
            y
        })
    }
}

/// Why an [`Image`] could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageError {
    /// The width or the height is zero.
    Empty,
    /// The width or the height is above [`MAX_DIMENSION`].
    TooLarge,
    /// The stride is less than the bytes of a row.
    StrideTooShort,
    /// The pixels end before the last row does.
    NotEnoughPixels,
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ImageError::Empty => "the image has no pixels",
            ImageError::TooLarge => "the image is wider or taller than the writers accept",
            ImageError::StrideTooShort => "the stride is shorter than a row",
            ImageError::NotEnoughPixels => "the pixels end before the last row",
        })
    }
}

impl std::error::Error for ImageError {}

/// A PNG row filter: what each byte is stored as the difference from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PngFilter {
    /// Nothing: the bytes as they are.
    None = 0,
    /// The byte one pixel to the left.
    Sub = 1,
    /// The byte one row up.
    Up = 2,
    /// The average of those two.
    Average = 3,
    /// Whichever of left, up and up-left is nearest the Paeth predictor.
    Paeth = 4,
}

/// The settings the original keeps in globals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WriteOptions {
    /// How hard the PNG writer's compressor searches for repeats
    /// (`stbi_write_png_compression_level`): see [`zlib_compress`]. 8 by default; below 5
    /// counts as 5.
    pub png_compression_level: u32,
    /// The filter for every row of a PNG, or `None` to try all five on each row and keep the
    /// one whose bytes are smallest (`stbi_write_force_png_filter`). `None` by default.
    pub png_filter: Option<PngFilter>,
    /// Write the image upside down: the last row first (`stbi_flip_vertically_on_write`).
    pub flip_vertically: bool,
}

impl Default for WriteOptions {
    fn default() -> WriteOptions {
        WriteOptions {
            png_compression_level: 8,
            png_filter: None,
            flip_vertically: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_are_checked_when_made() {
        let pixels = [0u8; 45];
        assert!(Image::new(&pixels, 5, 3, Channels::Rgb).is_ok());
        assert_eq!(
            Image::new(&pixels, 0, 3, Channels::Rgb).unwrap_err(),
            ImageError::Empty
        );
        assert_eq!(
            Image::new(&pixels[..44], 5, 3, Channels::Rgb).unwrap_err(),
            ImageError::NotEnoughPixels
        );
        assert_eq!(
            Image::with_stride(&pixels, 5, 3, Channels::Rgb, 14).unwrap_err(),
            ImageError::StrideTooShort
        );
        // The last row need not be followed by a whole stride.
        assert!(Image::with_stride(&pixels, 5, 2, Channels::Rgb, 30).is_ok());
        assert_eq!(
            Image::new(&pixels, MAX_DIMENSION + 1, 1, Channels::Grey).unwrap_err(),
            ImageError::TooLarge
        );
        assert_eq!(Channels::from_count(4), Some(Channels::Rgba));
        assert_eq!(Channels::from_count(5), None);
    }
}

/// The README's examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

/// The Guide's examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../docs/guide.md")]
struct GuideDoctests;
