// SPDX-License-Identifier: MIT OR Apache-2.0
//! Deterministic synthetic frames shared by the fixture generator and the tests.
//!
//! Integer arithmetic only, so the bytes are identical on every platform. None of these
//! resembles any real photograph, and none is derived from an upstream image.

fn xorshift(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

/// Packed RGB8 for one pattern.
pub fn make(width: u32, height: u32, pattern: u8) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut out = vec![0u8; w * h * 3];
    let mut rng = 0x9E37_79B9u32 ^ (width << 16) ^ height ^ (u32::from(pattern) << 8);
    for y in 0..h {
        for x in 0..w {
            let p = (y * w + x) * 3;
            let (r, g, b) = match pattern {
                // Blocks plus diagonal bands plus a little noise.
                0 => {
                    let blk = (((x / 9) ^ (y / 7)) * 37) & 255;
                    let band = ((x + 2 * y) * 3) & 255;
                    let n = (xorshift(&mut rng) & 15) as usize;
                    (blk, band, (blk + band + n) & 255)
                }
                // Rings and a ramp.
                1 => {
                    let dx = x as i64 - (w as i64) / 2;
                    let dy = y as i64 - (h as i64) / 2;
                    let d2 = (dx * dx + dy * dy) as usize;
                    let ring = (d2 / 37) & 1;
                    let ramp = (x * 255 / w.max(1)) & 255;
                    (
                        if ring == 1 { 220 } else { 30 },
                        ramp,
                        (y * 255 / h.max(1)) & 255,
                    )
                }
                // Mostly-flat regions with sparse edges.
                2 => {
                    let cell = ((x / 23) + (y / 31)) % 5;
                    let base = [20, 90, 160, 230, 60][cell];
                    let edge = if (x % 23 == 0) || (y % 31 == 0) {
                        80
                    } else {
                        0
                    };
                    (base, (base + edge) & 255, 255 - base)
                }
                // Flat (low quality).
                3 => (128, 128, 128),
                // Low-contrast random blocks: mid-range quality values (amplitude per pattern).
                5..=8 => {
                    let amp = [8usize, 24, 48, 12][usize::from(pattern) - 5];
                    let mut st = ((x / 8) as u32).wrapping_mul(0x85EB_CA6B)
                        ^ ((y / 8) as u32).wrapping_mul(0xC2B2_AE35)
                        ^ 0x1234_5678;
                    let lvl = 100 + (xorshift(&mut st) as usize % amp);
                    (lvl, lvl / 2 + 40, 200 - lvl / 3)
                }
                // Smooth gradient.
                _ => (
                    (x * 255 / w.max(1)) & 255,
                    (y * 255 / h.max(1)) & 255,
                    ((x + y) * 127 / (w + h).max(1)) & 255,
                ),
            };
            out[p] = r as u8;
            out[p + 1] = g as u8;
            out[p + 2] = b as u8;
        }
    }
    out
}
