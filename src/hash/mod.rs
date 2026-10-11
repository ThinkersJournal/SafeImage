//! Layer 1: perceptual hashing of a decoded frame. Pure, `no_std` plus `alloc`, no I/O.
//!
//! The only image input is a [`Frame`]: raw RGB8 with explicit dimensions. Decoding is the host's.
//! [`pdq`] returns one hash and its quality; [`pdq_dihedral`] returns the eight orientations from
//! one transform, which is what a caller matching rotated copies sends.
//!
//! The hashing code is a hand port of the Meta PDQ reference implementation; see `NOTICE`,
//! `docs/provenance/pdq-vetting.md` and `LICENSE` in this directory.

mod downscale;
mod frame;
mod hash256;
mod pdq;
mod torben;

pub use frame::{Frame, FrameError, MAX_FRAME_PIXELS};
pub use hash256::{Hash256, HexError};

/// Meta's recommended match distance: two hashes at or below this are "similar". A provider
/// applies its own threshold; this is the default for matching a list held locally.
///
/// A deliberate public item: the crate itself never applies it. Matching against a locally held
/// list is the caller's loop over [`Hash256::hamming_distance`], and this names the threshold that
/// loop should start from, so it is not a literal in every consumer. Exercised in
/// `tests/frame.rs`.
pub const MATCH_DISTANCE_DEFAULT: u32 = 31;

/// Meta's recommended quality floor: a hash with quality at or below this should be discarded.
pub const QUALITY_FLOOR_DEFAULT: u8 = 49;

/// One PDQ hash and its quality (0 to 100).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pdq {
    /// The 256-bit hash.
    pub hash: Hash256,
    /// Image-domain quality metric. At or below [`QUALITY_FLOOR_DEFAULT`] the hash is unreliable.
    pub quality: u8,
}

impl Pdq {
    /// True when quality is above `floor` (upstream recommends discarding hashes at or below it).
    pub fn is_usable(&self, floor: u8) -> bool {
        self.quality > floor
    }
}

/// The eight dihedral hashes of one frame, with the shared quality.
///
/// Order: original, rotate 90, rotate 180, rotate 270, flip X, flip Y, flip along the main
/// diagonal (plus 1), flip along the anti-diagonal (minus 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dihedral {
    /// The eight hashes in the order above.
    pub hashes: [Hash256; 8],
    /// Image-domain quality metric (shared; computed once).
    pub quality: u8,
}

/// Hashes one frame.
///
/// A frame with a side under 5 pixels hashes to the zero hash with quality 0, as the reference
/// does; callers must treat that as unusable (quality is at or below the floor). A frame of
/// exactly 64 x 64 skips the blur, as the reference does.
pub fn pdq(frame: &Frame<'_>) -> Pdq {
    let (mut b1, mut b2) = luma_buffers(frame);
    let (hash, quality) = pdq::pdq_hash256_from_float_luma(
        &mut b1,
        &mut b2,
        frame.height() as usize,
        frame.width() as usize,
    );
    Pdq { hash, quality }
}

/// Hashes one frame in all eight orientations.
pub fn pdq_dihedral(frame: &Frame<'_>) -> Dihedral {
    let (mut b1, mut b2) = luma_buffers(frame);
    let (hashes, quality) = pdq::pdq_dihedral_hash256es_from_float_luma(
        &mut b1,
        &mut b2,
        frame.height() as usize,
        frame.width() as usize,
    );
    Dihedral { hashes, quality }
}

fn luma_buffers(frame: &Frame<'_>) -> (alloc::vec::Vec<f32>, alloc::vec::Vec<f32>) {
    let n = frame.width() as usize * frame.height() as usize;
    let mut b1 = alloc::vec![0.0f32; n];
    let b2 = alloc::vec![0.0f32; n];
    pdq::fill_float_luma_from_rgb(
        frame.rgb(),
        frame.height() as usize,
        frame.width() as usize,
        &mut b1,
    );
    (b1, b2)
}

#[cfg(feature = "reference-support")]
#[doc(hidden)]
pub mod __reference_support {
    //! Not part of the API. Exposes intermediate values so the reference fixtures can compare
    //! them with the C++ reference's. Subject to change without notice.
    use alloc::vec::Vec;

    /// The 16 x 64 DCT matrix, row-major.
    pub fn dct_matrix_64() -> Vec<f32> {
        super::pdq::dct_matrix_64()
    }

    /// Luma plane of `frame` (`height * width`, row-major).
    pub fn luma(frame: &super::Frame<'_>) -> Vec<f32> {
        super::luma_buffers(frame).0
    }

    /// Reference `dct64To16` on a row-major 64 x 64 input; returns the 16 x 16 block, row-major.
    pub fn dct64_to_16(a: &[f32]) -> Vec<f32> {
        assert_eq!(a.len(), 64 * 64);
        let mut m = [[0.0f32; 64]; 64];
        for (i, row) in m.iter_mut().enumerate() {
            row.copy_from_slice(&a[i * 64..i * 64 + 64]);
        }
        let d = super::pdq::dct_matrix_64();
        super::pdq::dct64_to_16(&d, &m)
            .iter()
            .flatten()
            .copied()
            .collect()
    }

    /// Median by the reference's `torben`.
    pub fn torben(values: &[f32]) -> f32 {
        super::torben::torben(values)
    }
}
