//! Minimal PNG writer for 8-bit RGBA. No dependencies.

use crate::deflate;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
const BYTES_PER_PIXEL: usize = 4;

fn crc32(data: &[u8]) -> u32 {
    let step = |crc: u32| {
        if crc & 1 == 1 {
            (crc >> 1) ^ 0xedb88320
        } else {
            crc >> 1
        }
    };
    data.iter().fold(!0u32, |crc, &byte| {
        (0..8).fold(crc ^ byte as u32, |crc, _| step(crc))
    }) ^ !0u32
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    b << 16 | a
}

fn chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let tagged: Vec<u8> = kind.iter().chain(body).copied().collect();
    let lengths = (body.len() as u32).to_be_bytes();
    let check = crc32(&tagged).to_be_bytes();
    lengths.iter().chain(&tagged).chain(&check).copied().collect()
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let (pa, pb, pc) = ((p - a as i32).abs(), (p - b as i32).abs(), (p - c as i32).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn filtered(row: &[u8], above: &[u8], kind: u8) -> Vec<u8> {
    let left = |i: usize| if i >= BYTES_PER_PIXEL { row[i - BYTES_PER_PIXEL] } else { 0 };
    let upper_left =
        |i: usize| if i >= BYTES_PER_PIXEL { above[i - BYTES_PER_PIXEL] } else { 0 };
    let predict = |i: usize| match kind {
        1 => left(i),
        2 => above[i],
        3 => ((left(i) as u16 + above[i] as u16) / 2) as u8,
        _ => paeth(left(i), above[i], upper_left(i)),
    };
    (0..row.len()).map(|i| row[i].wrapping_sub(predict(i))).collect()
}

/// Standard minimum-sum-of-absolute-differences heuristic: the filter whose
/// output is closest to zero usually deflates best.
fn best_filter(row: &[u8], above: &[u8]) -> (u8, Vec<u8>) {
    let cost = |bytes: &[u8]| -> u32 {
        bytes.iter().map(|&b| (b as i8).unsigned_abs() as u32).sum()
    };
    let candidates = (1..=4).map(|kind| (kind, filtered(row, above, kind)));
    candidates
        .chain([(0, row.to_vec())])
        .min_by_key(|(_, bytes)| cost(bytes))
        .unwrap()
}

fn scanlines(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let stride = width * BYTES_PER_PIXEL;
    let mut raw = Vec::with_capacity((stride + 1) * height);
    let mut above = vec![0u8; stride];
    for y in 0..height {
        let row = &pixels[y * stride..(y + 1) * stride];
        let (kind, bytes) = best_filter(row, &above);
        raw.push(kind);
        raw.extend_from_slice(&bytes);
        above = row.to_vec();
    }
    raw
}

/// `pixels` is tightly packed RGBA, top row first.
pub fn encode(pixels: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);

    let raw = scanlines(pixels, width, height);
    let mut stream = vec![0x78, 0x01];
    stream.extend(deflate::compress(&raw));
    stream.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut png = SIGNATURE.to_vec();
    png.extend(chunk(b"IHDR", &header));
    png.extend(chunk(b"IDAT", &stream));
    png.extend(chunk(b"IEND", &[]));
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_the_published_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }

    #[test]
    fn adler_matches_the_published_check_value() {
        assert_eq!(adler32(b"123456789"), 0x091e01de);
    }

    #[test]
    fn encoding_starts_with_the_signature_and_ends_with_iend() {
        let png = encode(&[0u8; 4 * 4], 2, 2);
        assert_eq!(&png[..8], &SIGNATURE);
        assert_eq!(&png[png.len() - 12..], b"\0\0\0\0IEND\xae\x42\x60\x82");
    }
}
