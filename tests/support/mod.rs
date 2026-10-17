#![allow(
    dead_code,
    reason = "shared by several test crates; each uses a subset"
)]
//! Deterministic synthetic frames shared by the fixture generator and the tests.
//!
//! Integer arithmetic only, so the bytes are identical on every platform. None of these
//! resembles any real photograph, and none is derived from an upstream image.

/// (name, width, height, pattern)
pub const FRAMES: &[(&str, u32, u32, u8)] = &[
    ("tiny_4x4", 4, 4, 0),
    ("min_5x5", 5, 5, 0),
    ("small_7x9", 7, 9, 1),
    ("exact_64x64", 64, 64, 0),
    ("w100_h75", 100, 75, 1),
    ("sq128", 128, 128, 2),
    ("odd_255x257", 255, 257, 0),
    ("sq512", 512, 512, 1),
    ("w513_h300", 513, 300, 2),
    ("vga_640x480", 640, 480, 0),
    ("big_1024x768", 1024, 768, 1),
    ("flat_200x200", 200, 200, 3),
    ("gradient_300x200", 300, 200, 4),
    ("narrow_17x400", 17, 400, 2),
    ("soft_a8_128", 128, 128, 5),
    ("soft_a24_200x150", 200, 150, 6),
    ("soft_a48_320x240", 320, 240, 7),
    ("soft_a12_96x64", 96, 64, 8),
    // Above the first consumer's 512 px cap; the last is exactly `MAX_FRAME_PIXELS`.
    ("large_2048x1536", 2048, 1536, 0),
    ("large_3000x2001", 3000, 2001, 2),
    ("cap_4096x4096", 4096, 4096, 1),
];

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

/// The `dct64To16` input used by both sides (row-major 64 x 64).
pub fn dct_input() -> Vec<f32> {
    let mut a = vec![0f32; 64 * 64];
    for i in 0..64usize {
        for j in 0..64usize {
            a[i * 64 + j] = ((i * 131 + j * 71 + (i ^ j) * 17) % 255) as f32 / 3.0;
        }
    }
    a
}

/// Arrays for the `torben` median fixture (shared with the C++ harness by construction).
pub fn torben_cases() -> Vec<(&'static str, Vec<f32>)> {
    let ties: Vec<f32> = (0..256).map(|i| (i % 16) as f32).collect();
    let flat = vec![7.0f32; 256];
    let halves: Vec<f32> = (0..256).map(|i| if i < 128 { 0.0 } else { 1.0 }).collect();
    let mixed: Vec<f32> = (0..256usize)
        .map(|i| (((i * 2654435761usize) >> 7) % 1000) as f32 / 8.0 - 20.0)
        .collect();
    let odd: Vec<f32> = (0..255).map(|i| ((i * 37) % 101) as f32).collect();
    vec![
        ("ties", ties),
        ("flat", flat),
        ("halves", halves),
        ("mixed", mixed),
        ("odd", odd),
    ]
}
