use eyre::{Result, eyre};
use image::codecs::png::PngEncoder;
use image::{ImageBuffer, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Limits, Rgba};
use std::borrow::Cow;
use std::io::Cursor;

/// RGBA pixels shared by codecs and clipboard adapters without an OS dependency.
#[derive(Clone, Debug)]
pub struct ImageData<'a> {
    pub width: usize,
    pub height: usize,
    pub bytes: Cow<'a, [u8]>,
}

pub fn encode_png(image: ImageData<'static>) -> Result<Vec<u8>> {
    let width = image.width as u32;
    let height = image.height as u32;
    let bytes = image.bytes.into_owned();
    let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, bytes)
        .ok_or_else(|| eyre!("invalid image buffer"))?;

    let mut out = Vec::new();
    let encoder = PngEncoder::new(&mut out);
    encoder.write_image(
        buffer.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )?;
    Ok(out)
}

pub fn decode_png(data: &[u8], max_decoded_bytes: usize) -> Result<ImageData<'static>> {
    let mut reader = ImageReader::with_format(Cursor::new(data), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_alloc = Some(max_decoded_bytes as u64);
    reader.limits(limits);
    let decoder = reader
        .into_decoder()
        .map_err(|_| eyre!("png decode failed or image too large"))?;
    let (width, height) = decoder.dimensions();
    let decoded_bytes = (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(4);
    if decoded_bytes > max_decoded_bytes as u64 || decoder.total_bytes() > max_decoded_bytes as u64
    {
        return Err(eyre!("png image too large to decode safely"));
    }

    let image =
        image::DynamicImage::from_decoder(decoder).map_err(|_| eyre!("png decode failed"))?;
    let rgba = image.into_rgba8();
    let (width, height) = rgba.dimensions();
    let bytes = rgba.into_raw();
    Ok(ImageData {
        width: width as usize,
        height: height as usize,
        bytes: bytes.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_image() -> (ImageData<'static>, Vec<u8>) {
        let bytes = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        let image = ImageData {
            width: 2,
            height: 2,
            bytes: bytes.clone().into(),
        };
        (image, bytes)
    }

    #[test]
    fn encode_decode_round_trip() {
        let (image, original_bytes) = sample_image();
        let png = encode_png(image).unwrap();
        let decoded = decode_png(&png, 1024).unwrap();
        assert_eq!(decoded.width, 2);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.bytes.into_owned(), original_bytes);
    }

    #[test]
    fn decode_rejects_oversize() {
        let (image, _) = sample_image();
        let png = encode_png(image).unwrap();
        let err = decode_png(&png, 1).unwrap_err();
        assert!(err.to_string().contains("too large"));
    }

    #[test]
    fn decode_rejects_invalid_png() {
        let err = decode_png(b"not a png", 1024).unwrap_err();
        assert!(err.to_string().contains("png decode failed"));
    }

    #[test]
    fn sixteen_bit_png_native_output_is_bounded_even_when_rgba_dimensions_fit() {
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&[0; 18], 3, 1, image::ExtendedColorType::Rgb16)
            .unwrap();
        assert!(decode_png(&png, 12).is_err());
        assert_eq!(decode_png(&png, 1024).unwrap().bytes.len(), 12);
    }
}
