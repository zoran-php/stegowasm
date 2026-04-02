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
