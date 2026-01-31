use arboard::ImageData;
use eyre::{Result, eyre};
use image::codecs::png::{PngDecoder, PngEncoder};
use image::{ImageBuffer, ImageDecoder, ImageEncoder, ImageFormat, Rgba};
use std::io::Cursor;

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
    let decoder =
        PngDecoder::new(Cursor::new(data)).map_err(|err| eyre!("png decode failed: {err}"))?;
    let (width, height) = decoder.dimensions();
    let decoded_bytes = (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(4);
    if decoded_bytes > max_decoded_bytes as u64 {
        return Err(eyre!("png image too large to decode safely"));
    }

    let image = image::load_from_memory_with_format(data, ImageFormat::Png)
        .map_err(|err| eyre!("png decode failed: {err}"))?;
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
}
