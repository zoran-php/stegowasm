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

#[cfg(test)]
mod tests {
    use super::{
        FLAG_COMPRESSED, HEADER_SIZE_BITS, HEADER_SIZE_BYTES, capacity_bytes, compress, decompress,
        hide_bytes, read_bytes, read_header_and_payload, write_header_and_payload,
    };

    #[test]
    fn hides_and_reads_bytes_at_offset() {
        let mut image_bytes = vec![0u8; 64];
        let payload = b"OK";

        hide_bytes(&mut image_bytes, payload, 8).expect("payload should fit");

        let decoded = read_bytes(&image_bytes, payload.len(), 8).expect("payload should decode");
        assert_eq!(decoded, payload);
    }

    #[test]
    fn returns_error_when_image_capacity_is_too_small() {
        let mut image_bytes = vec![0u8; 7];
        let err = hide_bytes(&mut image_bytes, b"A", 0).expect_err("payload should not fit");
        assert!(err.contains("enough capacity"));
    }

    #[test]
    fn writes_and_reads_header_and_payload() {
        let payload = b"secret";
        let mut image_bytes = vec![0u8; HEADER_SIZE_BITS + payload.len() * 8];

        write_header_and_payload(&mut image_bytes, payload, FLAG_COMPRESSED)
            .expect("header and payload should fit");

        let (len, flags, decoded) =
            read_header_and_payload(&image_bytes).expect("embedded payload should decode");
        assert_eq!(len as usize, payload.len());
        assert_eq!(flags, FLAG_COMPRESSED);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn compresses_and_decompresses_round_trip() {
        let data = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let compressed = compress(data).expect("compression should succeed");
        let decompressed = decompress(&compressed).expect("decompression should succeed");

        assert_eq!(decompressed, data);
    }

    #[test]
    fn reports_capacity_in_bytes_from_image_length() {
        assert_eq!(capacity_bytes(0), 0);
        assert_eq!(capacity_bytes(8), 1);
        assert_eq!(capacity_bytes(300), 37);
        assert_eq!(HEADER_SIZE_BYTES, 5);
    }

    #[test]
    fn returns_error_when_read_bytes_exceeds_image_size() {
        let image_bytes = vec![0u8; 7];
        let err = read_bytes(&image_bytes, 1, 0)
            .expect_err("1 byte needs 8 bits but only 7 bytes available");
        assert!(err.contains("enough embedded data"));
    }
}
