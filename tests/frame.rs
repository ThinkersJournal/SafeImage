// SPDX-License-Identifier: MIT OR Apache-2.0
//! Frame construction and the "never panics" contract of layer 1.

mod support;

use safeimage::hash::{
    Frame, FrameError, Hash256, MATCH_DISTANCE_DEFAULT, MAX_FRAME_PIXELS, QUALITY_FLOOR_DEFAULT,
    pdq, pdq_dihedral,
};

#[test]
fn the_default_match_distance_is_an_inclusive_hamming_threshold() {
    // Two hashes differing in exactly `d` bits: the low `d` bits of the first words set.
    let differing = |d: u32| {
        let mut w = [0u16; 16];
        for k in 0..d as usize {
            w[k / 16] |= 1 << (k % 16);
        }
        Hash256::from_words(w)
    };
    let zero = Hash256::zero();
    assert_eq!(MATCH_DISTANCE_DEFAULT, 31);
    assert_eq!(
        zero.hamming_distance(&differing(MATCH_DISTANCE_DEFAULT)),
        MATCH_DISTANCE_DEFAULT
    );
    assert!(zero.hamming_distance(&differing(MATCH_DISTANCE_DEFAULT)) <= MATCH_DISTANCE_DEFAULT);
    assert!(zero.hamming_distance(&differing(MATCH_DISTANCE_DEFAULT + 1)) > MATCH_DISTANCE_DEFAULT);
}

#[test]
fn zero_dimensions_are_refused() {
    assert_eq!(
        Frame::new(&[], 0, 5).unwrap_err(),
        FrameError::ZeroDimension
    );
    assert_eq!(
        Frame::new(&[], 5, 0).unwrap_err(),
        FrameError::ZeroDimension
    );
}

#[test]
fn wrong_length_is_a_typed_error_not_a_hash() {
    let rgb = support::make(10, 10, 0);
    // Truncated by one byte, and one byte too many.
    assert_eq!(
        Frame::new(&rgb[..rgb.len() - 1], 10, 10).unwrap_err(),
        FrameError::LengthMismatch {
            expected: 300,
            actual: 299
        }
    );
    let mut long = rgb.clone();
    long.push(0);
    assert_eq!(
        Frame::new(&long, 10, 10).unwrap_err(),
        FrameError::LengthMismatch {
            expected: 300,
            actual: 301
        }
    );
    // Dimensions that do not describe the buffer (10 x 10 data read as 11 x 10).
    assert!(Frame::new(&rgb, 11, 10).is_err());
}

#[test]
fn oversized_and_overflowing_dimensions_are_refused_before_any_allocation() {
    assert_eq!(
        Frame::new(&[], u32::MAX, u32::MAX).unwrap_err(),
        FrameError::TooLarge
    );
    let side = (MAX_FRAME_PIXELS as f64).sqrt() as u32 + 1;
    assert_eq!(
        Frame::new(&[], side, side).unwrap_err(),
        FrameError::TooLarge
    );
}

#[test]
fn frames_below_the_minimum_hash_to_zero_with_quality_zero() {
    for (w, h) in [(1u32, 1u32), (4, 4), (4, 50), (50, 4)] {
        let rgb = support::make(w, h, 0);
        let f = Frame::new(&rgb, w, h).unwrap();
        let single = pdq(&f);
        assert_eq!(
            (single.hash.words(), single.quality),
            ([0u16; 16], 0),
            "{w}x{h}"
        );
        assert!(!single.is_usable(QUALITY_FLOOR_DEFAULT));
        let d = pdq_dihedral(&f);
        assert_eq!(d.quality, 0);
        assert!(d.hashes.iter().all(|x| x.words() == [0u16; 16]));
    }
}

#[test]
fn no_dimension_in_a_sweep_panics_and_quality_stays_in_range() {
    for w in 1u32..=70 {
        for h in [1u32, 4, 5, 6, 31, 63, 64, 65, 70] {
            let rgb = support::make(w, h, ((w + h) % 5) as u8);
            let f = Frame::new(&rgb, w, h).unwrap();
            assert!(pdq(&f).quality <= 100);
            assert!(pdq_dihedral(&f).quality <= 100);
        }
    }
    // Tall and wide extremes that make the box window larger than most rows.
    for (w, h) in [(5u32, 2000u32), (2000, 5), (129, 129), (257, 3)] {
        let rgb = support::make(w, h, 2);
        let f = Frame::new(&rgb, w, h).unwrap();
        let _ = pdq(&f);
        let _ = pdq_dihedral(&f);
    }
}

#[test]
fn frame_debug_output_never_contains_pixel_data() {
    let rgb = vec![0xABu8; 5 * 5 * 3];
    let f = Frame::new(&rgb, 5, 5).unwrap();
    let text = format!("{f:?}");
    assert!(text.contains("width: 5") && text.contains("bytes: 75"));
    assert!(!text.contains("171") && !text.to_lowercase().contains("ab"));
}

#[test]
fn flat_frames_are_low_quality() {
    let rgb = support::make(200, 200, 3);
    let f = Frame::new(&rgb, 200, 200).unwrap();
    assert!(!pdq(&f).is_usable(QUALITY_FLOOR_DEFAULT));
}

#[test]
fn identical_frames_are_distance_zero_and_a_changed_frame_is_not() {
    let a = support::make(300, 200, 4);
    let mut b = a.clone();
    for px in b.chunks_mut(3).take(300 * 100) {
        px[0] = 255 - px[0];
        px[1] = 255 - px[1];
    }
    let fa = Frame::new(&a, 300, 200).unwrap();
    let fb = Frame::new(&b, 300, 200).unwrap();
    assert_eq!(pdq(&fa).hash.hamming_distance(&pdq(&fa).hash), 0);
    assert!(pdq(&fa).hash.hamming_distance(&pdq(&fb).hash) > 0);
}

#[test]
fn to_bytes_is_word_zero_first_little_endian() {
    use safeimage::hash::Hash256;
    let mut w = [0u16; 16];
    w[0] = 0x0201;
    w[15] = 0xBEEF;
    let b = Hash256::from_words(w).to_bytes();
    assert_eq!(&b[..2], &[0x01, 0x02]);
    assert_eq!(&b[30..], &[0xEF, 0xBE]);
    assert_eq!(b.len(), 32);
}
