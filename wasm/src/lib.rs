//! Raw WebAssembly exports for SafeImage layer 1.
//!
//! No `wasm-bindgen`, no generated glue: the module has **no imports**, so a host instantiates
//! it synchronously from an already-compiled module (`new WebAssembly.Instance(module, {})`)
//! and calls the exports below. Memory is the module's own linear memory; the host allocates
//! with [`si_alloc`], writes the RGB bytes, calls [`si_pdq`] or [`si_pdq_dihedral`], reads the
//! output buffer, and frees with [`si_free`].
//!
//! Output layout, for `si_pdq`: 33 bytes = 32-byte hash (words 0..15, each little-endian, see
//! `Hash256::to_bytes`) followed by one quality byte. For `si_pdq_dihedral`: 8 hashes of 32
//! bytes in the order original, rotate 90, rotate 180, rotate 270, flip X, flip Y, flip plus 1,
//! flip minus 1, followed by one quality byte (257 bytes).
//!
//! Errors are returned as negative `i32` codes; a panic would abort the module (a trap) and the
//! functions are written so that no input reaches one.

#![allow(unsafe_code)] // pointer-taking C ABI; every unsafe block states its contract

use safeimage::hash::{Frame, FrameError, pdq, pdq_dihedral};

/// ABI version: bump when an export's signature or the output layout changes.
pub const ABI_VERSION: u32 = 1;

/// Success.
pub const OK: i32 = 0;
/// Width or height is zero.
pub const ERR_ZERO_DIMENSION: i32 = -1;
/// The frame exceeds the maximum pixel count.
pub const ERR_TOO_LARGE: i32 = -2;
/// The buffer length does not equal `width * height * 3`.
pub const ERR_LENGTH_MISMATCH: i32 = -3;
/// A null pointer was passed where bytes are required.
pub const ERR_NULL_POINTER: i32 = -4;
/// The frame was refused for a reason this module does not name (a newer core than this table).
pub const ERR_INVALID_FRAME: i32 = -5;

/// Bytes written by [`si_pdq`].
pub const PDQ_OUT_LEN: usize = 33;
/// Bytes written by [`si_pdq_dihedral`].
pub const DIHEDRAL_OUT_LEN: usize = 8 * 32 + 1;

/// ABI version of this module.
#[unsafe(no_mangle)]
pub extern "C" fn si_abi_version() -> u32 {
    ABI_VERSION
}

/// Allocates `len` zeroed bytes in module memory and returns the pointer, never null (a
/// zero-length request returns a pointer to a one-byte block, which `si_free` accepts with
/// `len = 0`).
#[unsafe(no_mangle)]
pub extern "C" fn si_alloc(len: usize) -> *mut u8 {
    let block = vec![0u8; len.max(1)].into_boxed_slice();
    Box::into_raw(block) as *mut u8
}

/// Frees a block from [`si_alloc`]. `len` must be the value passed to `si_alloc`.
///
/// # Safety
/// `ptr` must come from `si_alloc(len)` and must not be used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_free(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: the caller promises `ptr` is the pointer `si_alloc(len)` returned, so it is the
    // data pointer of a boxed slice of `len.max(1)` bytes, freed exactly once.
    unsafe {
        drop(Box::from_raw(core::ptr::slice_from_raw_parts_mut(
            ptr,
            len.max(1),
        )));
    }
}

fn frame_error_code(e: FrameError) -> i32 {
    match e {
        FrameError::ZeroDimension => ERR_ZERO_DIMENSION,
        FrameError::TooLarge => ERR_TOO_LARGE,
        FrameError::LengthMismatch { .. } => ERR_LENGTH_MISMATCH,
        // `FrameError` is non-exhaustive: a reason added later is still an error, never a hash.
        _ => ERR_INVALID_FRAME,
    }
}

/// Hashes one frame. Returns 0 and writes [`PDQ_OUT_LEN`] bytes at `out`, or a negative code.
///
/// # Safety
/// `rgb` must point to `rgb_len` readable bytes and `out` to [`PDQ_OUT_LEN`] writable bytes,
/// both inside this module's memory and not overlapping.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_pdq(
    rgb: *const u8,
    rgb_len: usize,
    width: u32,
    height: u32,
    out: *mut u8,
) -> i32 {
    if out.is_null() || (rgb.is_null() && rgb_len != 0) {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above; a zero length uses an empty slice instead of a possibly null ptr.
    let rgb = if rgb_len == 0 {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(rgb, rgb_len) }
    };
    let frame = match Frame::new(rgb, width, height) {
        Ok(f) => f,
        Err(e) => return frame_error_code(e),
    };
    let result = pdq(&frame);
    // SAFETY: contract above gives PDQ_OUT_LEN writable bytes at `out`.
    let out = unsafe { core::slice::from_raw_parts_mut(out, PDQ_OUT_LEN) };
    out[..32].copy_from_slice(&result.hash.to_bytes());
    out[32] = result.quality;
    OK
}

/// Hashes one frame in all eight orientations. Returns 0 and writes [`DIHEDRAL_OUT_LEN`] bytes
/// at `out`, or a negative code.
///
/// # Safety
/// As [`si_pdq`], with [`DIHEDRAL_OUT_LEN`] writable bytes at `out`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_pdq_dihedral(
    rgb: *const u8,
    rgb_len: usize,
    width: u32,
    height: u32,
    out: *mut u8,
) -> i32 {
    if out.is_null() || (rgb.is_null() && rgb_len != 0) {
        return ERR_NULL_POINTER;
    }
    // SAFETY: as in `si_pdq`.
    let rgb = if rgb_len == 0 {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(rgb, rgb_len) }
    };
    let frame = match Frame::new(rgb, width, height) {
        Ok(f) => f,
        Err(e) => return frame_error_code(e),
    };
    let result = pdq_dihedral(&frame);
    // SAFETY: contract above gives DIHEDRAL_OUT_LEN writable bytes at `out`.
    let out = unsafe { core::slice::from_raw_parts_mut(out, DIHEDRAL_OUT_LEN) };
    for (i, h) in result.hashes.iter().enumerate() {
        out[i * 32..i * 32 + 32].copy_from_slice(&h.to_bytes());
    }
    out[256] = result.quality;
    OK
}
