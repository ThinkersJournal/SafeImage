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

#![allow(
    unsafe_code,
    reason = "pointer-taking C ABI; every unsafe block states its contract"
)]

pub mod codec;
pub mod evidence;
pub mod evidence_codec;
pub mod report_codec;

use safeimage::hash::{Frame, FrameError, pdq, pdq_dihedral};
use safeimage::matcher::{
    MatchRequest, RetryDecision, RetryPolicy, Unavailable, classify_status, strongest,
};

use codec::{CodecError, unavailable_from_tag, unavailable_tag};

/// ABI version: bump when an export's signature or the output layout changes. 2 added the layer 2
/// exports, 3 the layer 4 exports, 4 the layer 3 exports; layer 1's layout is unchanged.
pub const ABI_VERSION: u32 = 4;

/// Largest structured input, in bytes. Inputs are bytes, not frames, so this is far below the
/// isolate memory limit; a larger buffer is refused before any decoding.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

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

/// A structured input is longer than [`MAX_INPUT_BYTES`].
pub const ERR_INPUT_TOO_LARGE: i32 = -6;
/// The output buffer is too small. For `si_strongest` the output never exceeds the input length.
pub const ERR_OUT_TOO_SMALL: i32 = -7;
/// The encoding version byte is not one this module reads.
pub const ERR_BAD_VERSION: i32 = -8;
/// The buffer ended inside a value.
pub const ERR_TRUNCATED: i32 = -9;
/// A tag this module does not know, where the value cannot be preserved. Treat as a refusal:
/// hold the image.
pub const ERR_UNKNOWN_TAG: i32 = -10;
/// A string was not UTF-8, or a flag byte was not 0 or 1.
pub const ERR_MALFORMED: i32 = -11;
/// Bytes remained after the value.
pub const ERR_TRAILING_BYTES: i32 = -12;
/// An argument was out of range (for example a request of zero or more than eight hashes).
pub const ERR_BAD_ARGUMENT: i32 = -13;

/// Bytes written by [`si_retry_decide`].
pub const RETRY_OUT_LEN: usize = 9;

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

pub(crate) fn codec_error_code(e: CodecError) -> i32 {
    match e {
        CodecError::BadVersion => ERR_BAD_VERSION,
        CodecError::Truncated => ERR_TRUNCATED,
        CodecError::UnknownTag => ERR_UNKNOWN_TAG,
        CodecError::Malformed => ERR_MALFORMED,
        CodecError::TrailingBytes => ERR_TRAILING_BYTES,
        CodecError::SamePerson => ERR_SAME_PERSON,
    }
}

/// Pure core of [`si_strongest`], so it can be tested without raw pointers.
pub fn strongest_bytes(input: &[u8], out_cap: usize) -> Result<Vec<u8>, i32> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ERR_INPUT_TOO_LARGE);
    }
    let list = codec::decode_outcomes(input).map_err(codec_error_code)?;
    let bytes = codec::encode_outcome(&strongest(list));
    if bytes.len() > out_cap {
        return Err(ERR_OUT_TOO_SMALL);
    }
    Ok(bytes)
}

/// Pure core of [`si_match_request`]: the number of hashes, or an error code.
pub fn match_request_len(input: &[u8]) -> Result<usize, i32> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ERR_INPUT_TOO_LARGE);
    }
    let hashes = codec::decode_hashes(input).map_err(codec_error_code)?;
    let n = hashes.len();
    MatchRequest::pdq(hashes)
        .map(|_| n)
        .map_err(|_| ERR_BAD_ARGUMENT)
}

/// Pure core of [`si_retry_decide`].
pub fn retry_bytes(
    max_retries: u32,
    delay_ms: u64,
    attempts_made: u32,
    reason_tag: u32,
    elapsed_ms: u64,
    deadline_ms: u64,
) -> [u8; RETRY_OUT_LEN] {
    // A tag outside a byte is unknown, and unknown is `MalformedResponse`: never retried.
    let reason = u8::try_from(reason_tag)
        .map(unavailable_from_tag)
        .unwrap_or(Unavailable::MalformedResponse);
    let policy = RetryPolicy {
        max_retries,
        delay_ms,
    };
    let mut out = [0u8; RETRY_OUT_LEN];
    if let RetryDecision::Retry { after_ms } =
        policy.decide(attempts_made, reason, elapsed_ms, deadline_ms)
    {
        out[0] = 1;
        out[1..].copy_from_slice(&after_ms.to_le_bytes());
    }
    out
}

/// Borrows `len` bytes at `ptr`, or the error code for a null pointer or an over-cap length.
///
/// # Safety
/// `ptr` must point to `len` readable bytes in this module's memory when non-null and `len > 0`.
pub(crate) unsafe fn input_slice<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], i32> {
    if len > MAX_INPUT_BYTES {
        return Err(ERR_INPUT_TOO_LARGE);
    }
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(ERR_NULL_POINTER);
    }
    // SAFETY: the caller's contract.
    Ok(unsafe { core::slice::from_raw_parts(ptr, len) })
}

/// The strongest of several outcomes (layer 2 `strongest`). Input: a version byte, a `u32` count,
/// then that many encoded outcomes (see `codec`). Writes the encoded strongest outcome (version
/// byte then the outcome; never longer than the input) at `out` and returns the byte count, or a
/// negative code. An empty list is `Unavailable(NotConfigured)`. An unknown outcome tag counts as
/// `Unavailable(MalformedResponse)`, never clean.
///
/// # Safety
/// `input` must point to `input_len` readable bytes and `out` to `out_cap` writable bytes, both
/// inside this module's memory and not overlapping.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_strongest(
    input: *const u8,
    input_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above.
    let input = match unsafe { input_slice(input, input_len) } {
        Ok(s) => s,
        Err(c) => return c,
    };
    match strongest_bytes(input, out_cap) {
        Ok(bytes) => {
            // SAFETY: `bytes.len() <= out_cap`, and the contract gives `out_cap` writable bytes.
            let dst = unsafe { core::slice::from_raw_parts_mut(out, bytes.len()) };
            dst.copy_from_slice(&bytes);
            bytes.len() as i32
        }
        Err(c) => c,
    }
}

/// Maps an HTTP status to the reason it is unavailable. Returns 0 for a 2xx status, otherwise
/// 1 plus the unavailable tag (0 NotConfigured, 1 Disabled, 2 Timeout, 3 ServerError,
/// 4 Throttled, 5 CredentialsRefused, 6 MalformedResponse, 7 TestValueInProduction). A value that
/// is not a `u16` is a malformed response.
#[unsafe(no_mangle)]
pub extern "C" fn si_classify_status(status: u32) -> i32 {
    match u16::try_from(status) {
        Ok(s) => match classify_status(s) {
            None => 0,
            Some(u) => 1 + i32::from(unavailable_tag(u)),
        },
        Err(_) => 1 + i32::from(unavailable_tag(Unavailable::MalformedResponse)),
    }
}

/// 1 when a body of `body_len` bytes is within `cap`, else 0 (layer 2 `within_cap`).
#[unsafe(no_mangle)]
pub extern "C" fn si_within_cap(body_len: usize, cap: usize) -> i32 {
    i32::from(body_len <= cap)
}

/// Retry decision (layer 2 `RetryPolicy::decide`). Writes [`RETRY_OUT_LEN`] bytes at `out`: byte 0
/// is 1 to retry or 0 to stop, bytes 1 to 8 the delay in milliseconds, little-endian (0 on stop).
/// `reason_tag` is the unavailable tag of `si_classify_status`. Returns [`RETRY_OUT_LEN`], or a
/// negative code.
///
/// # Safety
/// `out` must point to [`RETRY_OUT_LEN`] writable bytes in this module's memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_retry_decide(
    max_retries: u32,
    delay_ms: u64,
    attempts_made: u32,
    reason_tag: u32,
    elapsed_ms: u64,
    deadline_ms: u64,
    out: *mut u8,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let bytes = retry_bytes(
        max_retries,
        delay_ms,
        attempts_made,
        reason_tag,
        elapsed_ms,
        deadline_ms,
    );
    // SAFETY: contract above.
    unsafe { core::slice::from_raw_parts_mut(out, RETRY_OUT_LEN) }.copy_from_slice(&bytes);
    RETRY_OUT_LEN as i32
}

/// Validates a request (layer 2 `MatchRequest::pdq`): a version byte, a `u32` count, then that
/// many 32-byte hashes in the layer 1 layout. Returns the hash count (1 to 8), or a negative code
/// (`ERR_BAD_ARGUMENT` for zero or more than eight).
///
/// # Safety
/// `input` must point to `input_len` readable bytes in this module's memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_match_request(input: *const u8, input_len: usize) -> i32 {
    // SAFETY: contract above.
    let input = match unsafe { input_slice(input, input_len) } {
        Ok(s) => s,
        Err(c) => return c,
    };
    match match_request_len(input) {
        Ok(n) => n as i32,
        Err(c) => c,
    }
}

// ---- Layer 4: the generic report mechanism ----------------------------------------------------
//
// Every export takes encoded messages (see `report_codec`) and writes one at `out`, returning the
// byte count or a negative code. `si_report_apply` may write up to its state input plus
// `REPORT_APPLY_HEADROOM` bytes (a capped code, a capped event detail and a moved report id).

/// Two actor ids that must differ were equal (the two-person rule).
pub const ERR_SAME_PERSON: i32 = -16;

/// Extra output bytes `si_report_apply` may need beyond the state it was given.
pub const REPORT_APPLY_HEADROOM: usize = 1024;
/// Most files a new report may declare.
pub const MAX_REPORT_FILES: u32 = 65_536;
/// A withdrawal was requested for a report that already finished.
pub const ERR_ALREADY_FINISHED: i32 = -14;
/// The core refused for a reason this module does not name (a newer core than this table).
pub const ERR_REFUSED: i32 = -15;

pub(crate) fn encode_checked(bytes: Vec<u8>, out_cap: usize) -> Result<Vec<u8>, i32> {
    if bytes.len() > out_cap {
        Err(ERR_OUT_TOO_SMALL)
    } else {
        Ok(bytes)
    }
}

fn state_in(input: &[u8]) -> Result<safeimage::report::ReportState, i32> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(ERR_INPUT_TOO_LARGE);
    }
    report_codec::decode_state(input).map_err(codec_error_code)
}

/// Pure core of [`si_report_new`].
pub fn report_new_bytes(
    file_count: u32,
    credentials_ready: u32,
    out_cap: usize,
) -> Result<Vec<u8>, i32> {
    if file_count > MAX_REPORT_FILES || credentials_ready > 1 {
        return Err(ERR_BAD_ARGUMENT);
    }
    let s = safeimage::report::ReportState::new(file_count as usize, credentials_ready == 1);
    encode_checked(report_codec::encode_state(&s), out_cap)
}

/// Pure core of [`si_report_next_step`].
pub fn report_next_step_bytes(state: &[u8], now: u64, out_cap: usize) -> Result<Vec<u8>, i32> {
    let s = state_in(state)?;
    encode_checked(
        report_codec::encode_step(&safeimage::report::next_step(&s, now)),
        out_cap,
    )
}

/// Pure core of [`si_report_apply`].
pub fn report_apply_bytes(
    state: &[u8],
    step: &[u8],
    answer: &[u8],
    now: u64,
    out_cap: usize,
) -> Result<Vec<u8>, i32> {
    let s = state_in(state)?;
    if step.len() > MAX_INPUT_BYTES || answer.len() > MAX_INPUT_BYTES {
        return Err(ERR_INPUT_TOO_LARGE);
    }
    let step = report_codec::decode_step(step).map_err(codec_error_code)?;
    let answer = report_codec::decode_answer(answer).map_err(codec_error_code)?;
    let (next, event) = safeimage::report::apply(&s, &step, &answer, now);
    encode_checked(report_codec::encode_applied(&next, &event), out_cap)
}

/// Pure core of [`si_report_sync_credentials`].
pub fn report_sync_bytes(state: &[u8], ready: u32, out_cap: usize) -> Result<Vec<u8>, i32> {
    if ready > 1 {
        return Err(ERR_BAD_ARGUMENT);
    }
    let s = state_in(state)?;
    let n = safeimage::report::sync_credentials(&s, ready == 1);
    encode_checked(report_codec::encode_state(&n), out_cap)
}

/// Pure core of [`si_report_request_retract`].
pub fn report_retract_bytes(state: &[u8], out_cap: usize) -> Result<Vec<u8>, i32> {
    let s = state_in(state)?;
    match safeimage::report::request_retract(&s) {
        Ok(n) => encode_checked(report_codec::encode_state(&n), out_cap),
        Err(safeimage::report::RetractError::AlreadyFinished) => Err(ERR_ALREADY_FINISHED),
        Err(_) => Err(ERR_REFUSED),
    }
}

/// Pure core of [`si_report_retry_failed`].
pub fn report_retry_bytes(state: &[u8], out_cap: usize) -> Result<Vec<u8>, i32> {
    let s = state_in(state)?;
    encode_checked(
        report_codec::encode_state(&safeimage::report::retry_failed(&s)),
        out_cap,
    )
}

/// Writes `bytes` at `out` and returns the count, or the error code.
///
/// # Safety
/// `out` must point to at least `bytes.len()` writable bytes (callers pass `out_cap` and the pure
/// cores have already checked `bytes.len() <= out_cap`).
pub(crate) unsafe fn emit(out: *mut u8, result: Result<Vec<u8>, i32>) -> i32 {
    match result {
        Ok(bytes) => {
            // SAFETY: the caller's contract.
            unsafe { core::slice::from_raw_parts_mut(out, bytes.len()) }.copy_from_slice(&bytes);
            bytes.len() as i32
        }
        Err(c) => c,
    }
}

/// A new report: `file_count` files, `credentials_ready` 0 or 1. Writes the encoded state.
///
/// # Safety
/// `out` must point to `out_cap` writable bytes in this module's memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_new(
    file_count: u32,
    credentials_ready: u32,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above; the core checked the length against `out_cap`.
    unsafe {
        emit(
            out,
            report_new_bytes(file_count, credentials_ready, out_cap),
        )
    }
}

/// What is due next for an encoded state at time `now` (seconds). Writes the encoded step.
///
/// # Safety
/// `state` must point to `state_len` readable bytes and `out` to `out_cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_next_step(
    state: *const u8,
    state_len: usize,
    now: u64,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above.
    let state = match unsafe { input_slice(state, state_len) } {
        Ok(s) => s,
        Err(c) => return c,
    };
    // SAFETY: contract above.
    unsafe { emit(out, report_next_step_bytes(state, now, out_cap)) }
}

/// Applies the destination's answer to a step. Writes the new state followed by an optional event
/// (one flag byte, then the event); allocate the state length plus [`REPORT_APPLY_HEADROOM`].
///
/// # Safety
/// Each input pointer must point to its length in readable bytes and `out` to `out_cap` writable
/// bytes, all inside this module's memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_apply(
    state: *const u8,
    state_len: usize,
    step: *const u8,
    step_len: usize,
    answer: *const u8,
    answer_len: usize,
    now: u64,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above, for each of the three inputs.
    let (state, step, answer) = unsafe {
        match (
            input_slice(state, state_len),
            input_slice(step, step_len),
            input_slice(answer, answer_len),
        ) {
            (Ok(a), Ok(b), Ok(c)) => (a, b, c),
            (Err(c), _, _) | (_, Err(c), _) | (_, _, Err(c)) => return c,
        }
    };
    // SAFETY: contract above.
    unsafe { emit(out, report_apply_bytes(state, step, answer, now, out_cap)) }
}

/// Moves a report between awaiting-credentials and pending. `ready` is 0 or 1.
///
/// # Safety
/// As [`si_report_next_step`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_sync_credentials(
    state: *const u8,
    state_len: usize,
    ready: u32,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above.
    let state = match unsafe { input_slice(state, state_len) } {
        Ok(s) => s,
        Err(c) => return c,
    };
    // SAFETY: contract above.
    unsafe { emit(out, report_sync_bytes(state, ready, out_cap)) }
}

/// Requests a withdrawal. [`ERR_ALREADY_FINISHED`] when the report already finished.
///
/// # Safety
/// As [`si_report_next_step`] without `now`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_request_retract(
    state: *const u8,
    state_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above.
    let state = match unsafe { input_slice(state, state_len) } {
        Ok(s) => s,
        Err(c) => return c,
    };
    // SAFETY: contract above.
    unsafe { emit(out, report_retract_bytes(state, out_cap)) }
}

/// A human's retry of a failed report.
///
/// # Safety
/// As [`si_report_request_retract`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_retry_failed(
    state: *const u8,
    state_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    // SAFETY: contract above.
    let state = match unsafe { input_slice(state, state_len) } {
        Ok(s) => s,
        Err(c) => return c,
    };
    // SAFETY: contract above.
    unsafe { emit(out, report_retry_bytes(state, out_cap)) }
}

/// Seconds to wait after `attempts` consecutive failures (layer 4 `backoff_secs`).
#[unsafe(no_mangle)]
pub extern "C" fn si_backoff_secs(attempts: u32) -> u64 {
    safeimage::report::backoff_secs(attempts)
}

/// 1 when an unfinished report may have been discarded by the destination, else 0
/// (layer 4 `deletion_window_elapsed`).
#[unsafe(no_mangle)]
pub extern "C" fn si_deletion_window_elapsed(
    opened_at: u64,
    last_modified_at: u64,
    now: u64,
    open_secs: u64,
    idle_secs: u64,
) -> i32 {
    i32::from(safeimage::report::deletion_window_elapsed(
        opened_at,
        last_modified_at,
        now,
        safeimage::report::WindowPolicy {
            open_secs,
            idle_secs,
        },
    ))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_evidence;

#[cfg(test)]
mod tests_report;
