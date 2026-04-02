use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use std::io::{Read, Write};

pub const HEADER_SIZE_BYTES: usize = 5; // 4 bytes length + 1 byte flags
pub const HEADER_SIZE_BITS: usize = HEADER_SIZE_BYTES * 8;
pub const FLAG_COMPRESSED: u8 = 0b0000_0001;

pub fn capacity_bytes(image_bytes_len: usize) -> usize {
    image_bytes_len / 8
}

pub fn hide_bytes(image_bytes: &mut [u8], addition: &[u8], offset: usize) -> Result<(), String> {
    let required_bits = addition.len() * 8;
    if offset + required_bits > image_bytes.len() {
        return Err("Image does not have enough capacity".to_string());
    }

    let mut pos = offset;
    for &byte in addition {
        for bit in (0..8).rev() {
            let b = (byte >> bit) & 1;
            image_bytes[pos] = (image_bytes[pos] & 0xFE) | b;
            pos += 1;
        }
    }

    Ok(())
}

pub fn read_bytes(image_bytes: &[u8], len: usize, offset: usize) -> Result<Vec<u8>, String> {
    let required_bits = len * 8;
    if offset + required_bits > image_bytes.len() {
        return Err("Image does not contain enough embedded data".to_string());
    }

    let mut pos = offset;
    let mut out = vec![0u8; len];

    for item in &mut out {
        let mut value = 0u8;
        for _ in 0..8 {
            value = (value << 1) | (image_bytes[pos] & 1);
            pos += 1;
        }
        *item = value;
    }

    Ok(out)
}

pub fn write_header_and_payload(
    image_bytes: &mut [u8],
    payload: &[u8],
    flags: u8,
) -> Result<(), String> {
    let len_u32 = u32::try_from(payload.len()).map_err(|_| "Payload too large")?;
    let mut header = Vec::with_capacity(HEADER_SIZE_BYTES);
    header.extend_from_slice(&len_u32.to_be_bytes());
    header.push(flags);

    hide_bytes(image_bytes, &header, 0)?;
    hide_bytes(image_bytes, payload, HEADER_SIZE_BITS)?;
    Ok(())
}

pub fn read_header_and_payload(image_bytes: &[u8]) -> Result<(u32, u8, Vec<u8>), String> {
    let header = read_bytes(image_bytes, HEADER_SIZE_BYTES, 0)?;
    let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]);
    let flags = header[4];

    let payload = read_bytes(image_bytes, length as usize, HEADER_SIZE_BITS)?;
    Ok((length, flags, payload))
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|e| format!("Compression write failed: {e}"))?;
    encoder
        .finish()
        .map_err(|e| format!("Compression finish failed: {e}"))
}

pub fn decompress(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|e| format!("Decompression failed: {e}"))?;
    Ok(out)
}
