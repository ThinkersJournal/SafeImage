// Derived from facebook/ThreatExchange pdq/cpp/hashing/pdqhashing.cpp at commit
// 07b82cb6e87b7e0ac7fc2a01d865df5db10ee1f2 (blob 82c5e7132c51ae6c7bb73ac645b3903f88b155e1).
// Original: Copyright (c) Meta Platforms, Inc. and affiliates.
// Licence: see LICENSE in this directory.
//
// Algorithm description (upstream): pdqhash-2017-10-09.pdf in facebookexternal/ThreatExchange-PDQ.
// Function names follow upstream (snake-cased) so a reviewer can diff them by eye. Every
// floating-point operation keeps the upstream's type (f32 unless upstream uses double) and
// order, because the output is compared bit for bit against the reference.

use alloc::vec;
use alloc::vec::Vec;

use super::downscale::{compute_jarosz_filter_window_size, decimate_float, jarosz_filter_float};
use super::hash256::Hash256;
use super::torben::torben;

// From Wikipedia: standard RGB to luminance (the 'Y' in 'YUV'). Upstream `const float`
// initialised from double literals.
const LUMA_FROM_R_COEFF: f32 = 0.299;
const LUMA_FROM_G_COEFF: f32 = 0.587;
const LUMA_FROM_B_COEFF: f32 = 0.114;

/// Upstream `MIN_HASHABLE_DIM`: smaller frames hash to zero with quality 0.
pub(crate) const MIN_HASHABLE_DIM: usize = 5;

/// Upstream `PDQ_NUM_JAROSZ_XY_PASSES` (tent filter).
const PDQ_NUM_JAROSZ_XY_PASSES: usize = 2;

const DCT_ROWS: usize = 16;
const DCT_COLS: usize = 64;

/// Upstream `dct_matrix_64`: 16 x 64, row-major, built in double and stored as float.
///
/// `matrix_scale_factor` is a `const float` upstream, so the double square root is rounded to
/// `f32` first; the product with the double cosine is then formed in double.
pub(crate) fn dct_matrix_64() -> Vec<f32> {
    const PI: f64 = core::f64::consts::PI;
    let matrix_scale_factor = libm::sqrt(2.0 / DCT_COLS as f64) as f32;
    let mut m = vec![0.0f32; DCT_ROWS * DCT_COLS];
    for i in 0..DCT_ROWS {
        for j in 0..DCT_COLS {
            let arg = (PI / 2.0 / DCT_COLS as f64) * (i + 1) as f64 * (2 * j + 1) as f64;
            m[i * DCT_COLS + j] = (f64::from(matrix_scale_factor) * libm::cos(arg)) as f32;
        }
    }
    m
}

/// Upstream `fillFloatLumaFromRGB` for packed RGB8 (row stride `3 * cols`, column stride 3).
///
/// Upstream defines this function twice at the pinned commit (`downscaling.cpp` and
/// `pdqhashing.cpp`); the reference fixtures prove the two give identical output on every
/// fixture frame, so there is one implementation here.
pub(crate) fn fill_float_luma_from_rgb(
    rgb: &[u8],
    num_rows: usize,
    num_cols: usize,
    luma: &mut [f32],
) {
    for i in 0..num_rows {
        for j in 0..num_cols {
            let p = (i * num_cols + j) * 3;
            let r = f32::from(rgb[p]);
            let g = f32::from(rgb[p + 1]);
            let b = f32::from(rgb[p + 2]);
            luma[i * num_cols + j] =
                LUMA_FROM_R_COEFF * r + LUMA_FROM_G_COEFF * g + LUMA_FROM_B_COEFF * b;
        }
    }
}

/// Upstream `pdqImageDomainQualityMetric`, clamped to 0..=100 as upstream does.
pub(crate) fn pdq_image_domain_quality_metric(b: &[[f32; 64]; 64]) -> u8 {
    let mut gradient_sum: i32 = 0;
    for rows in b.windows(2) {
        for (u, v) in rows[0].iter().zip(rows[1].iter()) {
            let d = (((u - v) * 100.0f32) / 255.0f32) as i32;
            gradient_sum += d.abs();
        }
    }
    for row in b.iter() {
        for pair in row.windows(2) {
            let d = (((pair[0] - pair[1]) * 100.0f32) / 255.0f32) as i32;
            gradient_sum += d.abs();
        }
    }
    // Heuristic scaling factor.
    let mut quality = gradient_sum / 90;
    if quality > 100 {
        quality = 100;
    }
    quality as u8
}

/// Upstream `dct64To16`: `B = D A Dt` for the first 16 frequencies, accumulating in `f32` in
/// upstream's order.
pub(crate) fn dct64_to_16(d: &[f32], a: &[[f32; 64]; 64]) -> [[f32; 16]; 16] {
    let mut t = [[0.0f32; 64]; 16];
    for (i, trow) in t.iter_mut().enumerate() {
        for (j, tv) in trow.iter_mut().enumerate() {
            let mut sumk = 0.0f32;
            for k in 0..64 {
                sumk += d[i * 64 + k] * a[k][j];
            }
            *tv = sumk;
        }
    }
    let mut b = [[0.0f32; 16]; 16];
    for (i, brow) in b.iter_mut().enumerate() {
        for (j, bv) in brow.iter_mut().enumerate() {
            let mut sumk = 0.0f32;
            for k in 0..64 {
                sumk += t[i][k] * d[j * 64 + k];
            }
            *bv = sumk;
        }
    }
    b
}

/// Upstream `pdqBuffer16x16ToBits`: bit `i*16+j` is set when the coefficient is above the median.
pub(crate) fn pdq_buffer_16x16_to_bits(dct: &[[f32; 16]; 16]) -> Hash256 {
    let mut flat = [0.0f32; 256];
    for (dst, src) in flat.iter_mut().zip(dct.iter().flatten()) {
        *dst = *src;
    }
    let dct_median = torben(&flat);
    let mut hash = Hash256::zero();
    for (i, row) in dct.iter().enumerate() {
        for (j, v) in row.iter().enumerate() {
            if *v > dct_median {
                hash.set_bit(i * 16 + j);
            }
        }
    }
    hash
}

/// The eight dihedral transforms of a 16 x 16 DCT block (upstream `dct16OriginalTo*`), in the
/// order of [`crate::hash::Dihedral::hashes`].
pub(crate) fn dihedral_blocks(a: &[[f32; 16]; 16]) -> [[[f32; 16]; 16]; 8] {
    let mut out = [[[0.0f32; 16]; 16]; 8];
    for i in 0..16 {
        for j in 0..16 {
            let v = a[i][j];
            // 0 original
            out[0][i][j] = v;
            // 1 rotate 90
            out[1][j][i] = if j & 1 != 0 { v } else { -v };
            // 2 rotate 180
            out[2][i][j] = if (i + j) & 1 != 0 { -v } else { v };
            // 3 rotate 270
            out[3][j][i] = if i & 1 != 0 { v } else { -v };
            // 4 flip X
            out[4][i][j] = if i & 1 != 0 { v } else { -v };
            // 5 flip Y
            out[5][i][j] = if j & 1 != 0 { v } else { -v };
            // 6 flip plus-1 (transpose)
            out[6][j][i] = v;
            // 7 flip minus-1
            out[7][j][i] = if (i + j) & 1 != 0 { -v } else { v };
        }
    }
    out
}

/// Box-filters and decimates a luma buffer to 64 x 64 (the shared first half of upstream
/// `pdqFloat256FromFloatLuma` and `pdqDihedralHash256esFromFloatLuma`).
fn downsample_to_64(
    buffer1: &mut [f32],
    buffer2: &mut [f32],
    num_rows: usize,
    num_cols: usize,
) -> [[f32; 64]; 64] {
    let window_size_along_rows = compute_jarosz_filter_window_size(num_cols, 64);
    let window_size_along_cols = compute_jarosz_filter_window_size(num_rows, 64);
    jarosz_filter_float(
        buffer1,
        buffer2,
        num_rows,
        num_cols,
        window_size_along_rows,
        window_size_along_cols,
        PDQ_NUM_JAROSZ_XY_PASSES,
    );
    let mut flat = vec![0.0f32; 64 * 64];
    decimate_float(buffer1, num_rows, num_cols, &mut flat, 64, 64);
    let mut out = [[0.0f32; 64]; 64];
    for (i, row) in out.iter_mut().enumerate() {
        row.copy_from_slice(&flat[i * 64..i * 64 + 64]);
    }
    out
}

/// Upstream `pdqHash256FromFloatLuma`: one hash and its quality. Consumes `buffer1`/`buffer2`.
pub(crate) fn pdq_hash256_from_float_luma(
    buffer1: &mut [f32],
    buffer2: &mut [f32],
    num_rows: usize,
    num_cols: usize,
) -> (Hash256, u8) {
    if num_rows < MIN_HASHABLE_DIM || num_cols < MIN_HASHABLE_DIM {
        return (Hash256::zero(), 0);
    }
    let buffer64 = if num_rows == 64 && num_cols == 64 {
        // E.g. for video-frame processing when we've already downsampled: no blur.
        let mut out = [[0.0f32; 64]; 64];
        for (k, v) in buffer1.iter().enumerate().take(64 * 64) {
            out[k / 64][k % 64] = *v;
        }
        out
    } else {
        downsample_to_64(buffer1, buffer2, num_rows, num_cols)
    };
    let quality = pdq_image_domain_quality_metric(&buffer64);
    let d = dct_matrix_64();
    let b16 = dct64_to_16(&d, &buffer64);
    (pdq_buffer_16x16_to_bits(&b16), quality)
}

/// Upstream `pdqDihedralHash256esFromFloatLuma`: the eight hashes and the quality.
pub(crate) fn pdq_dihedral_hash256es_from_float_luma(
    buffer1: &mut [f32],
    buffer2: &mut [f32],
    num_rows: usize,
    num_cols: usize,
) -> ([Hash256; 8], u8) {
    if num_rows < MIN_HASHABLE_DIM || num_cols < MIN_HASHABLE_DIM {
        return ([Hash256::zero(); 8], 0);
    }
    // Upstream always blurs here, including for a 64 x 64 input (a window of 1 is the identity).
    let buffer64 = downsample_to_64(buffer1, buffer2, num_rows, num_cols);
    let quality = pdq_image_domain_quality_metric(&buffer64);
    let d = dct_matrix_64();
    let b16 = dct64_to_16(&d, &buffer64);
    let blocks = dihedral_blocks(&b16);
    let mut hashes = [Hash256::zero(); 8];
    for (h, block) in hashes.iter_mut().zip(blocks.iter()) {
        *h = pdq_buffer_16x16_to_bits(block);
    }
    (hashes, quality)
}
