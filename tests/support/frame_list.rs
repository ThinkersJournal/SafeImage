// SPDX-License-Identifier: MIT OR Apache-2.0
//! The synthetic frame list shared by the reference test and the fixture generator.

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
