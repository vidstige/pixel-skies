//! Stacks straight-alpha RGBA images, blending in sRGB like common image
//! tools do.

fn blend(top: &[u8], bottom: &[u8]) -> [u8; 4] {
    let a = top[3] as u32;
    let mix = |i: usize| ((top[i] as u32 * a + bottom[i] as u32 * (255 - a) + 127) / 255) as u8;
    [mix(0), mix(1), mix(2), bottom[3]]
}

/// `top` over an opaque `bottom`, both tightly packed RGBA.
pub fn over(top: &[u8], bottom: &[u8]) -> Vec<u8> {
    let pixels = top.chunks_exact(4).zip(bottom.chunks_exact(4));
    pixels.flat_map(|(t, b)| blend(t, b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKY: [u8; 4] = [200, 120, 40, 255];

    fn black(alpha: u8) -> [u8; 4] {
        [0, 0, 0, alpha]
    }

    #[test]
    fn clear_pixels_show_what_is_below_and_opaque_ones_hide_it() {
        assert_eq!(over(&black(0), &SKY), SKY);
        assert_eq!(over(&black(255), &SKY), black(255));
    }

    #[test]
    fn half_cover_darkens_by_half() {
        let half = over(&black(128), &SKY);
        for c in 0..3 {
            assert!((half[c] as f64 - SKY[c] as f64 / 2.0).abs() <= 1.0);
        }
        assert_eq!(half[3], 255);
    }
}
