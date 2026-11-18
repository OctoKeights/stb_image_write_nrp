//! The BMP writer (`stbi_write_bmp_core`).

use crate::{Channels, Image, WriteOptions};

/// The bytes of a `.bmp` file of `image` (`stbi_write_bmp_to_func`).
///
/// One to three channels are written as 24 bits a pixel (grey as three equal values, with the
/// alpha of [`Channels::GreyAlpha`] dropped), and [`Channels::Rgba`] as 32 bits with alpha,
/// behind a version 4 header.
#[must_use]
pub fn write_bmp(image: &Image, options: &WriteOptions) -> Vec<u8> {
    let (width, height) = (image.width, image.height);
    let rgba = image.channels == Channels::Rgba;
    // Rows are padded to four bytes; with alpha they already are.
    let (pixel_bytes, pad) = if rgba {
        (4, 0)
    } else {
        (3, (width.wrapping_neg().wrapping_mul(3) & 3) as usize)
    };
    let header_len: u32 = if rgba { 14 + 108 } else { 14 + 40 };
    let row_len = width as usize * pixel_bytes + pad;
    let file_len = header_len.wrapping_add((row_len as u32).wrapping_mul(height));

    let mut out = Vec::with_capacity(header_len as usize + row_len * height as usize);
    // The file header.
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_len.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&header_len.to_le_bytes());
    // The bitmap header.
    let mut put = |value: u32| out.extend_from_slice(&value.to_le_bytes());
    put(header_len - 14);
    put(width);
    put(height);
    put(1 | (pixel_bytes as u32 * 8) << 16); // planes, and bits a pixel
    if rgba {
        // BI_BITFIELDS with masks for each channel, alpha among them, since most readers
        // ignore the alpha of plain BI_RGB; then the colour space, "sRGB".
        for value in [
            3,
            0,
            0,
            0,
            0,
            0,
            0xFF_0000,
            0xFF00,
            0xFF,
            0xFF00_0000,
            0x7352_4742,
        ] {
            put(value);
        }
        for _ in 0..12 {
            put(0);
        }
    } else {
        for _ in 0..6 {
            put(0);
        }
    }

    // The rows from the bottom, each pixel blue, green, red (and alpha).
    let n = image.channels.count();
    for y in 0..height as usize {
        let row = image.file_row(y, !options.flip_vertically);
        match image.channels {
            Channels::Grey | Channels::GreyAlpha => {
                for pixel in row.chunks_exact(n) {
                    out.extend_from_slice(&[pixel[0]; 3]);
                }
            }
            Channels::Rgb => {
                for pixel in row.as_chunks::<3>().0 {
                    out.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
                }
            }
            Channels::Rgba => {
                for pixel in row.as_chunks::<4>().0 {
                    out.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                }
            }
        }
        out.extend_from_slice(&[0; 3][..pad]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bmp_is_laid_out_bottom_up() {
        let options = WriteOptions::default();
        let pixels: Vec<u8> = (0..30).map(|i| i * 7).collect();
        let file = write_bmp(&Image::new(&pixels, 5, 2, Channels::Rgb).unwrap(), &options);
        // Rows of 15 bytes padded to 16, behind 54 bytes of headers.
        assert_eq!(file.len(), 54 + 2 * 16);
        assert_eq!(&file[..2], b"BM");
        assert_eq!(u32::from_le_bytes(file[2..6].try_into().unwrap()), 86);
        assert_eq!(u16::from_le_bytes(file[28..30].try_into().unwrap()), 24);
        // The last row first, blue before red.
        assert_eq!(&file[54..57], &[pixels[17], pixels[16], pixels[15]]);
        assert_eq!(file[54 + 15], 0);
        // Grey is three equal values; four channels keep their alpha behind a larger header.
        let grey = write_bmp(
            &Image::new(&[7, 9], 2, 1, Channels::Grey).unwrap(),
            &options,
        );
        assert_eq!(&grey[54..], &[7, 7, 7, 9, 9, 9, 0, 0]);
        let rgba = write_bmp(
            &Image::new(&[1, 2, 3, 4], 1, 1, Channels::Rgba).unwrap(),
            &options,
        );
        assert_eq!((rgba.len(), &rgba[122..]), (126, &[3u8, 2, 1, 4][..]));
    }
}
