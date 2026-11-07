//! Deflate with fixed Huffman codes and greedy LZ77. No dependencies.
//!
//! Only the compressor side exists, which is why the fixed code tables are
//! enough: dynamic codes would buy another 10% or so on top of a match
//! finder that is already doing most of the work.

const WINDOW: usize = 32768;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
/// Hash chain positions inspected per byte. Beyond this the ratio barely
/// moves but the cost keeps climbing.
const MAX_CHAIN: usize = 32;
const HASH_BITS: usize = 15;
/// Chain terminator, distinct from any real position.
const NONE: u32 = u32::MAX;

const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u32; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [usize; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u32; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    accumulator: u32,
    count: u32,
}

impl Bits {
    /// Deflate packs its own fields low bit first.
    fn push(&mut self, value: u32, width: u32) {
        self.accumulator |= value << self.count;
        self.count += width;
        while self.count >= 8 {
            self.bytes.push(self.accumulator as u8);
            self.accumulator >>= 8;
            self.count -= 8;
        }
    }

    /// Huffman codes travel high bit first, against the stream's own order.
    fn push_code(&mut self, code: u32, width: u32) {
        let reversed = (0..width).fold(0, |acc, i| acc | ((code >> i) & 1) << (width - 1 - i));
        self.push(reversed, width);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.bytes.push(self.accumulator as u8);
        }
        self.bytes
    }
}

fn push_literal(bits: &mut Bits, byte: u8) {
    match byte {
        0..=143 => bits.push_code(0x30 + byte as u32, 8),
        _ => bits.push_code(0x190 + byte as u32 - 144, 9),
    }
}

fn symbol(bases: &[usize], value: usize) -> usize {
    bases.iter().rposition(|&base| base <= value).unwrap()
}

fn push_match(bits: &mut Bits, length: usize, distance: usize) {
    let l = symbol(&LENGTH_BASE, length);
    match l {
        0..=22 => bits.push_code(1 + l as u32, 7),
        _ => bits.push_code(0xc0 + l as u32 - 23, 8),
    }
    bits.push((length - LENGTH_BASE[l]) as u32, LENGTH_EXTRA[l]);
    let d = symbol(&DISTANCE_BASE, distance);
    bits.push_code(d as u32, 5);
    bits.push((distance - DISTANCE_BASE[d]) as u32, DISTANCE_EXTRA[d]);
}

fn hash(data: &[u8], at: usize) -> usize {
    let key = (data[at] as usize) << 16 | (data[at + 1] as usize) << 8 | data[at + 2] as usize;
    (key.wrapping_mul(2654435761)) >> (32 - HASH_BITS) & ((1 << HASH_BITS) - 1)
}

fn match_length(data: &[u8], a: usize, b: usize) -> usize {
    let limit = MAX_MATCH.min(data.len() - b);
    (0..limit).take_while(|&i| data[a + i] == data[b + i]).count()
}

/// Longest match for the string at `at`, searching the hash chain back to
/// the start of the window. `NONE` ends a chain.
fn longest_match(data: &[u8], previous: &[u32], head: u32, at: usize) -> (usize, usize) {
    let mut candidate = head;
    let mut best = (0, 0);
    for _ in 0..MAX_CHAIN {
        if candidate == NONE || at - candidate as usize > WINDOW {
            break;
        }
        let length = match_length(data, candidate as usize, at);
        if length > best.0 {
            best = (length, at - candidate as usize);
        }
        candidate = previous[candidate as usize];
    }
    best
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits::default();
    bits.push(1, 1);
    bits.push(1, 2);
    let mut heads = vec![NONE; 1 << HASH_BITS];
    let mut previous = vec![NONE; data.len()];
    let mut at = 0;
    while at + MIN_MATCH <= data.len() {
        let slot = hash(data, at);
        let (length, distance) = longest_match(data, &previous, heads[slot], at);
        previous[at] = heads[slot];
        heads[slot] = at as u32;
        if length >= MIN_MATCH {
            push_match(&mut bits, length, distance);
            // Chain entries for the skipped bytes would pay for themselves
            // only on highly repetitive input; hashing just the head is the
            // usual trade.
            at += length;
        } else {
            push_literal(&mut bits, data[at]);
            at += 1;
        }
    }
    for &byte in &data[at..] {
        push_literal(&mut bits, byte);
    }
    bits.push_code(0, 7);
    bits.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repetitive_input_compresses_far_below_its_size() {
        let data = "the sky at sunset, ".repeat(500).into_bytes();
        assert!(compress(&data).len() < data.len() / 20);
    }

    #[test]
    fn incompressible_input_stays_close_to_its_size() {
        let hash = |i: u32| (i.wrapping_mul(2654435761) >> 24) as u8;
        let data: Vec<u8> = (0..4096).map(hash).collect();
        assert!(compress(&data).len() < data.len() * 10 / 8);
    }
}
