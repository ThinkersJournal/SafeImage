// SPDX-License-Identifier: MIT OR Apache-2.0
//! The frame-size rule a host's decoder applies before handing the crate a [`Frame`](super::Frame).

/// The longest side, in pixels, a decoder should deliver. A frame whose sides are both at or
/// below this is hashed at its native size.
pub const NORMALISED_MAX_SIDE: u32 = 512;

/// The size a decoder should deliver for a source of `width` x `height`.
///
/// The rule: a source whose longer side is at most [`NORMALISED_MAX_SIDE`] keeps its size (no
/// upscale). A larger source is scaled so its longer side is exactly [`NORMALISED_MAX_SIDE`], the
/// shorter side scaled by the same ratio, rounded half up and never below 1. A zero side is
/// returned unchanged: there is no frame to size.
///
/// This is arithmetic only. The crate resizes nothing and decodes nothing; the host's decoder does
/// the resize and may check its output against this function.
pub fn normalise_geometry(width: u32, height: u32) -> (u32, u32) {
    let long = width.max(height);
    if width == 0 || height == 0 || long <= NORMALISED_MAX_SIDE {
        return (width, height);
    }
    let scale = |side: u32| -> u32 {
        let s = u64::from(side) * u64::from(NORMALISED_MAX_SIDE);
        let l = u64::from(long);
        (((2 * s + l) / (2 * l)) as u32).max(1)
    };
    let w = if width == long {
        NORMALISED_MAX_SIDE
    } else {
        scale(width)
    };
    let h = if height == long {
        NORMALISED_MAX_SIDE
    } else {
        scale(height)
    };
    (w, h)
}
