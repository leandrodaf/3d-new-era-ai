//! Bounded decoding shared by the editor, image renders and video renders.
use std::io::Cursor;

use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, ImageResult};

/// Per-image working payload, including RGBA/display conversion copies.
pub const MAX_IMAGE_BYTES: u64 = if cfg!(target_arch = "wasm32") {
    64 * 1024 * 1024
} else {
    256 * 1024 * 1024
};

fn memory_error() -> ImageError {
    ImageError::Limits(image::error::LimitError::from_kind(
        image::error::LimitErrorKind::InsufficientMemory,
    ))
}

fn cost(decoder: &impl ImageDecoder) -> ImageResult<u64> {
    let (w, h) = decoder.dimensions();
    u64::from(w)
        .checked_mul(u64::from(h))
        .and_then(|pixels| pixels.checked_mul(8))
        .and_then(|copies| copies.checked_add(decoder.total_bytes()))
        .ok_or_else(memory_error)
}

fn decoder(bytes: &[u8], format: ImageFormat) -> ImageResult<impl ImageDecoder + '_> {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(MAX_IMAGE_BYTES);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    if cost(&decoder)? > MAX_IMAGE_BYTES {
        return Err(memory_error());
    }
    // Keep decoder scratch allocations separate from its output allocation.
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;
    Ok(decoder)
}

/// Decode only after dimensions and output/conversion payload fit the cap.
///
/// # Errors
/// Invalid, unsupported or oversized images are refused before pixel decoding.
pub fn decode(bytes: &[u8]) -> ImageResult<DynamicImage> {
    DynamicImage::from_decoder(decoder(bytes, image::guess_format(bytes)?)?)
}

/// Header-based payload estimate. Non-image assets return `None`.
///
/// # Errors
/// Recognized images with invalid headers or excessive dimensions/memory fail.
pub fn estimated_memory(bytes: &[u8]) -> ImageResult<Option<u64>> {
    let Ok(format) = image::guess_format(bytes) else {
        return Ok(None);
    };
    cost(&decoder(bytes, format)?).map(Some)
}

/// Check the aggregate referenced image payload before dispatching a render.
///
/// # Errors
/// A damaged/oversized image or aggregate payload above `limit` is reported.
pub fn validate_assets<'a>(
    assets: impl IntoIterator<Item = (&'a str, &'a [u8])>,
    limit: u64,
) -> Result<(), String> {
    let mut total = 0u64;
    for (path, bytes) in assets {
        let image = estimated_memory(bytes).map_err(|e| {
            format!("Imagem {path} não pode ser carregada dentro do limite de memória: {e}")
        })?;
        total = total
            .checked_add(image.unwrap_or(0))
            .ok_or("Image memory overflow")?;
        if total > limit {
            return Err(format!(
                "Imagens decodificadas excedem o limite de {limit} bytes; reduza as texturas do projeto."
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::ImageEncoder;

    fn small_png() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&[255; 16], 2, 2, image::ExtendedColorType::Rgba8)
            .unwrap();
        bytes
    }

    #[test]
    fn oversized_dimensions_fail_as_limits_before_attempting_pixel_decode() {
        // Valid PNG header claiming 24000² pixels, with only a tiny payload.
        // A decoder reaching pixels would fail as corrupt data, after trying
        // to allocate the enormous image. The limit must reject it first.
        let bytes = include_bytes!("../tests/fixtures/oversized-header.png");
        assert!(matches!(decode(bytes), Err(ImageError::Limits(_))));
        assert!(matches!(
            estimated_memory(bytes),
            Err(ImageError::Limits(_))
        ));
    }

    #[test]
    fn decoded_payload_includes_conversion_copies_and_aggregate_limits() {
        let bytes = small_png();
        assert_eq!(estimated_memory(&bytes).unwrap(), Some(48));
        assert_eq!(decode(&bytes).unwrap().to_rgba8().into_raw(), vec![255; 16]);
        assert!(
            validate_assets(
                [("a.png", bytes.as_slice()), ("b.png", bytes.as_slice())],
                96
            )
            .is_ok()
        );
        assert!(
            validate_assets(
                [("a.png", bytes.as_slice()), ("b.png", bytes.as_slice())],
                95
            )
            .is_err()
        );
        assert_eq!(estimated_memory(b"v 0 0 0\n").unwrap(), None);
        assert!(validate_assets([("model.obj", b"v 0 0 0\n".as_slice())], 0).is_ok());
    }
}
