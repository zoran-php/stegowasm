mod crypto;
mod stego;
mod utils;

use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use stego::{
    FLAG_COMPRESSED, HEADER_SIZE_BYTES, capacity_bytes, compress, decompress,
    read_header_and_payload, write_header_and_payload,
};
use utils::js_err;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

fn encode_payload(
    raw_text_bytes: &[u8],
    image_capacity_bytes: usize,
    use_encryption: bool,
    password: Option<&str>,
) -> Result<(Vec<u8>, u8), String> {
    let available_payload_bytes = image_capacity_bytes
        .checked_sub(HEADER_SIZE_BYTES)
        .ok_or_else(|| "Image too small to hold header".to_string())?;

    let raw_candidate = if use_encryption {
        let pwd = password
            .ok_or_else(|| "Password is required when encryption is enabled".to_string())?;
        crypto::encrypt_bytes(raw_text_bytes, pwd)?
    } else {
        raw_text_bytes.to_vec()
    };

    if raw_candidate.len() <= available_payload_bytes {
        return Ok((raw_candidate, 0));
    }

    let compressed = compress(raw_text_bytes)?;
    let compressed_candidate = if use_encryption {
        let pwd = password
            .ok_or_else(|| "Password is required when encryption is enabled".to_string())?;
        crypto::encrypt_bytes(&compressed, pwd)?
    } else {
        compressed
    };

    if compressed_candidate.len() <= available_payload_bytes {
        return Ok((compressed_candidate, FLAG_COMPRESSED));
    }

    Err(format!(
        "Text does not fit in image. Capacity: {} bytes payload, raw candidate: {} bytes, compressed candidate: {} bytes",
        available_payload_bytes,
        raw_candidate.len(),
        compressed_candidate.len()
    ))
}

#[wasm_bindgen]
pub fn embed_text(
    png_bytes: &[u8],
    text: &str,
    use_encryption: bool,
    password: Option<String>,
) -> Result<Vec<u8>, JsValue> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| js_err(format!("Failed to decode PNG: {e}")))?;

    let mut rgb = img.to_rgb8();
    let image_bytes = rgb.as_mut();
    let capacity = capacity_bytes(image_bytes.len());

    let (payload, flags) = encode_payload(
        text.as_bytes(),
        capacity,
        use_encryption,
        password.as_deref(),
    )
    .map_err(js_err)?;

    write_header_and_payload(image_bytes, &payload, flags).map_err(js_err)?;

    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(rgb)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|e| js_err(format!("Failed to encode PNG: {e}")))?;

    Ok(out.into_inner())
}

#[wasm_bindgen]
pub fn extract_text(
    png_bytes: &[u8],
    use_encryption: bool,
    password: Option<String>,
) -> Result<String, JsValue> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| js_err(format!("Failed to decode PNG: {e}")))?;

    let rgb = img.to_rgb8();
    let image_bytes = rgb.as_raw();

    let (_len, flags, mut payload) = read_header_and_payload(image_bytes).map_err(js_err)?;

    if use_encryption {
        let pwd = password
            .as_deref()
            .ok_or_else(|| js_err("Password is required when encryption is enabled"))?;
        payload = crypto::decrypt_bytes(&payload, pwd).map_err(js_err)?;
    }

    if (flags & FLAG_COMPRESSED) != 0 {
        payload = decompress(&payload).map_err(js_err)?;
    }

    String::from_utf8(payload).map_err(|e| js_err(format!("Invalid UTF-8 payload: {e}")))
}

#[wasm_bindgen]
pub fn estimate_capacity(png_bytes: &[u8]) -> Result<u32, JsValue> {
    let img = image::load_from_memory(png_bytes)
        .map_err(|e| js_err(format!("Failed to decode PNG: {e}")))?;
    let rgb = img.to_rgb8();
    let cap = capacity_bytes(rgb.as_raw().len())
        .checked_sub(HEADER_SIZE_BYTES)
        .ok_or_else(|| js_err("Image too small to hold header"))?;
    u32::try_from(cap).map_err(|_| js_err("Capacity overflow"))
}

#[cfg(test)]
mod tests {
    use super::{embed_text, encode_payload, estimate_capacity, extract_text};
    use crate::stego::{
        FLAG_COMPRESSED, HEADER_SIZE_BYTES, capacity_bytes, read_header_and_payload,
    };
    use image::{DynamicImage, ImageFormat, RgbImage};
    use std::io::Cursor;

    fn make_png(width: u32, height: u32) -> Vec<u8> {
        let rgb = RgbImage::from_fn(width, height, |x, y| {
            let base = ((x + y) % 255) as u8;
            image::Rgb([base, base.wrapping_add(40), base.wrapping_add(80)])
        });

        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(rgb)
            .write_to(&mut out, ImageFormat::Png)
            .expect("PNG generation should succeed");
        out.into_inner()
    }

    fn make_mostly_incompressible_text(len: usize) -> String {
        let mut x = 0x1234_5678u32;
        let mut out = String::with_capacity(len);
        for _ in 0..len {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let ch = b'!' + (x % 94) as u8;
            out.push(char::from(ch));
        }
        out
    }

    #[test]
    fn embeds_and_extracts_plain_text() {
        let input = make_png(16, 16);
        let text = "Secret message";

        let encoded =
            embed_text(&input, text, false, None).expect("embedding plain text should succeed");
        let decoded =
            extract_text(&encoded, false, None).expect("extracting plain text should succeed");

        assert_eq!(decoded, text);
    }

    #[test]
    fn embeds_and_extracts_encrypted_text() {
        let input = make_png(24, 24);
        let text = "encrypted message";

        let encoded = embed_text(&input, text, true, Some("hunter2".to_string()))
            .expect("embedding encrypted text should succeed");
        let decoded = extract_text(&encoded, true, Some("hunter2".to_string()))
            .expect("extracting encrypted text should succeed");

        assert_eq!(decoded, text);
    }

    #[test]
    fn sets_compression_flag_when_raw_text_does_not_fit() {
        let input = make_png(10, 10);
        let text = "A".repeat(100);

        let encoded =
            embed_text(&input, &text, false, None).expect("compressed embedding should succeed");
        let img = image::load_from_memory(&encoded).expect("encoded PNG should decode");
        let rgb = img.to_rgb8();
        let (_len, flags, payload) = read_header_and_payload(rgb.as_raw())
            .expect("embedded header and payload should decode");

        assert_ne!(flags & FLAG_COMPRESSED, 0);
        assert!(payload.len() < text.len());
    }

    #[test]
    fn estimates_capacity_from_png_size() {
        let input = make_png(16, 16);
        let img = image::load_from_memory(&input).expect("PNG should decode");
        let rgb = img.to_rgb8();
        let expected = capacity_bytes(rgb.as_raw().len()) - HEADER_SIZE_BYTES;

        let actual = estimate_capacity(&input).expect("capacity estimation should succeed");

        assert_eq!(actual as usize, expected);
    }

    #[test]
    fn rejects_payload_that_cannot_fit_even_after_compression() {
        let text = make_mostly_incompressible_text(256);
        let image_capacity_bytes = capacity_bytes(10 * 10 * 3);
        let err = encode_payload(text.as_bytes(), image_capacity_bytes, false, None)
            .expect_err("oversized payload should be rejected");

        assert!(err.contains("Text does not fit in image"));
    }

    #[test]
    fn requires_password_when_encryption_is_enabled() {
        let err =
            encode_payload(b"secret", 128, true, None).expect_err("password should be required");
        assert!(err.contains("Password is required"));
    }

    #[test]
    fn rejects_image_too_small_to_hold_header_in_encode_payload() {
        let err = encode_payload(b"hi", 3, false, None)
            .expect_err("capacity below header size should be rejected");
        assert!(err.contains("too small to hold header"));
    }

    #[test]
    fn embeds_and_extracts_compressed_encrypted_text() {
        // 16x16 image: 768 raw bytes, capacity = 96, available = 91.
        // "B" * 60 unencrypted = 60 bytes, which fits raw without encryption,
        // but encrypted (60 + ~44 overhead = ~104 bytes) exceeds 91, forcing
        // the compressed+encrypted code path.
        let input = make_png(16, 16);
        let text = "B".repeat(60);

        let encoded = embed_text(&input, &text, true, Some("pass".to_string()))
            .expect("compressed encrypted embedding should succeed");
        let decoded = extract_text(&encoded, true, Some("pass".to_string()))
            .expect("compressed encrypted extraction should succeed");

        assert_eq!(decoded, text);
    }
}
