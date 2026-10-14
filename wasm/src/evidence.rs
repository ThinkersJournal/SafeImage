//! Layer 3 exports: evidence states, the case record, the preservation clock and the two-person
//! rule. Pure and profile-independent; storage, transactions and the identity behind an actor id
//! stay in the host (`docs/WASM-BOUNDARY.md` section 2).
//!
//! Calling convention: structured inputs are encoded messages ([`crate::evidence_codec`]); a
//! function that returns a case writes the new encoded case at `out` and returns its length, and
//! a function that answers a question returns a small non-negative code. A refusal is a negative
//! code; the table is `ERR_*` in the crate root and the `CASE_*`, `INELIGIBLE_*` and `RELEASE_*`
//! constants here. **No function here deletes anything**: destruction is only recorded.

use safeimage::evidence::{
    self as ev, CaseError, CaseKind, HoldTarget, Ineligible, ReleaseRefusal,
};

use crate::codec::CodecError;
use crate::evidence_codec as ec;
use crate::{
    ERR_BAD_ARGUMENT, ERR_INPUT_TOO_LARGE, ERR_NULL_POINTER, ERR_REFUSED, MAX_INPUT_BYTES,
    codec_error_code, emit, encode_checked, input_slice,
};

/// Extra output bytes a case-writing export may need beyond the encoded case it was given, on top
/// of the length of any reason or request reference passed in.
pub const CASE_HEADROOM: usize = 1024;

/// The evidence table lists no such transition.
pub const ERR_TRANSITION_REFUSED: i32 = -25;
/// Destruction has begun: the case refuses every other write.
pub const CASE_DESTROYED: i32 = -20;
/// The case already has a decision.
pub const CASE_ALREADY_DECIDED: i32 = -21;
/// This kind of case cannot be cleared.
pub const CASE_NOT_CLEARABLE: i32 = -22;
/// This kind of case needs two different people to clear.
pub const CASE_NEEDS_TWO_PEOPLE: i32 = -23;
/// No file at that index.
pub const CASE_NO_SUCH_FILE: i32 = -24;
/// A legal hold is already set.
pub const CASE_HOLD_ALREADY_SET: i32 = -26;
/// No legal hold to lift.
pub const CASE_NO_HOLD: i32 = -27;
/// A reason or reference was blank.
pub const CASE_BLANK: i32 = -28;
/// Destruction has already begun.
pub const CASE_DESTRUCTION_ALREADY_BEGUN: i32 = -29;
/// The reaper is not eligible: undecided.
pub const INELIGIBLE_UNDECIDED: i32 = -31;
/// The reaper is not eligible: a confirmed case's evidence is kept.
pub const INELIGIBLE_CONFIRMED: i32 = -32;
/// The reaper is not eligible: before the preservation end, or the clock says keep everything.
pub const INELIGIBLE_BEFORE_END: i32 = -33;
/// The reaper is not eligible: an evidence legal hold applies.
pub const INELIGIBLE_LEGAL_HOLD: i32 = -34;
/// The reaper is not eligible: a live or confirmed case shares a file.
pub const INELIGIBLE_SHARED_FILE: i32 = -35;
/// The reaper is not eligible: destruction has already begun.
pub const INELIGIBLE_ALREADY_BEGUN: i32 = -36;
/// A media hold may not be released: the case is not cleared.
pub const RELEASE_NOT_CLEARED: i32 = -40;
/// A media hold may not be released: the evidence copy is not yet held and verified.
pub const RELEASE_EVIDENCE_COPY_NOT_HELD: i32 = -41;
/// A media hold may not be released: before the preservation end.
pub const RELEASE_BEFORE_END: i32 = -42;
/// A media hold may not be released: an evidence legal hold applies.
pub const RELEASE_LEGAL_HOLD: i32 = -43;

/// Bytes written by [`si_preserve_until`].
pub const PRESERVE_OUT_LEN: usize = 9;

fn ineligible_code(i: Ineligible) -> i32 {
    match i {
        Ineligible::Undecided => INELIGIBLE_UNDECIDED,
        Ineligible::Confirmed => INELIGIBLE_CONFIRMED,
        Ineligible::BeforeEnd => INELIGIBLE_BEFORE_END,
        Ineligible::LegalHold => INELIGIBLE_LEGAL_HOLD,
        Ineligible::SharedFile => INELIGIBLE_SHARED_FILE,
        Ineligible::AlreadyBegun => INELIGIBLE_ALREADY_BEGUN,
        // A reason added by a newer core: still a refusal.
        _ => ERR_REFUSED,
    }
}

fn case_error_code(e: CaseError) -> i32 {
    match e {
        CaseError::Destroyed => CASE_DESTROYED,
        CaseError::AlreadyDecided => CASE_ALREADY_DECIDED,
        CaseError::NotClearable(_) => CASE_NOT_CLEARABLE,
        CaseError::NeedsTwoPeople(_) => CASE_NEEDS_TWO_PEOPLE,
        CaseError::NoSuchFile(_) => CASE_NO_SUCH_FILE,
        CaseError::Transition(_) => ERR_TRANSITION_REFUSED,
        CaseError::HoldAlreadySet => CASE_HOLD_ALREADY_SET,
        CaseError::NoHold => CASE_NO_HOLD,
        CaseError::Blank(_) => CASE_BLANK,
        CaseError::DestructionAlreadyBegun => CASE_DESTRUCTION_ALREADY_BEGUN,
        CaseError::NotEligible(i) => ineligible_code(i),
        _ => ERR_REFUSED,
    }
}

fn release_code(r: ReleaseRefusal) -> i32 {
    match r {
        ReleaseRefusal::NotCleared => RELEASE_NOT_CLEARED,
        ReleaseRefusal::EvidenceCopyNotHeld => RELEASE_EVIDENCE_COPY_NOT_HELD,
        ReleaseRefusal::BeforeEnd => RELEASE_BEFORE_END,
        ReleaseRefusal::LegalHold => RELEASE_LEGAL_HOLD,
        _ => ERR_REFUSED,
    }
}

fn dec<T>(r: Result<T, CodecError>) -> Result<T, i32> {
    r.map_err(codec_error_code)
}

fn flag(v: u32) -> Result<bool, i32> {
    match v {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(ERR_BAD_ARGUMENT),
    }
}

fn opt_u64(present: u32, v: u64) -> Result<Option<u64>, i32> {
    Ok(if flag(present)? { Some(v) } else { None })
}

fn capped(b: &[u8]) -> Result<&[u8], i32> {
    if b.len() > MAX_INPUT_BYTES {
        Err(ERR_INPUT_TOO_LARGE)
    } else {
        Ok(b)
    }
}

fn text(b: &[u8]) -> Result<&str, i32> {
    core::str::from_utf8(capped(b)?).map_err(|_| crate::ERR_MALFORMED)
}

fn case_in(b: &[u8]) -> Result<ev::CaseRecord, i32> {
    dec(ec::decode_case(capped(b)?))
}

fn case_out(r: Result<ev::CaseRecord, CaseError>, cap: usize) -> Result<Vec<u8>, i32> {
    match r {
        Ok(c) => encode_checked(ec::encode_case(&c), cap),
        Err(e) => Err(case_error_code(e)),
    }
}

// ---- pure cores (tested without pointers) -----------------------------------------------------

/// Pure core of [`si_evidence_transition`]: the new state tag.
pub fn transition_code(state: u32, event: &[u8]) -> Result<i32, i32> {
    let from = dec(ec::state_from_tag(
        u8::try_from(state).map_err(|_| ERR_BAD_ARGUMENT)?,
    ))?;
    let event = dec(ec::decode_event(capped(event)?))?;
    match ev::transition(from, &event) {
        Ok(s) => Ok(i32::from(ec::state_tag(s))),
        Err(_) => Err(ERR_TRANSITION_REFUSED),
    }
}

/// Pure core of [`si_case_evidence`]: 0 failed, 1 pending, 2 present.
pub fn case_evidence_code(states: &[u8]) -> Result<i32, i32> {
    let states = dec(ec::decode_states(capped(states)?))?;
    Ok(match ev::case_evidence(&states) {
        ev::CaseEvidence::Failed => 0,
        ev::CaseEvidence::Pending => 1,
        ev::CaseEvidence::Present => 2,
    })
}

/// Pure core of [`si_report_file_disposition`]: 0 send with file, 1 send without, 2 hold.
pub fn disposition_code(state: u32, found: u32) -> Result<i32, i32> {
    let s = dec(ec::state_from_tag(
        u8::try_from(state).map_err(|_| ERR_BAD_ARGUMENT)?,
    ))?;
    Ok(match ev::report_file_disposition(s, flag(found)?) {
        ev::FileDisposition::SendWithFile => 0,
        ev::FileDisposition::SendWithoutFile => 1,
        ev::FileDisposition::Hold => 2,
    })
}

/// Pure core of [`si_report_may_submit`].
pub fn may_submit_code(files: &[u8]) -> Result<i32, i32> {
    let files = dec(ec::decode_state_found(capped(files)?))?;
    Ok(i32::from(ev::report_may_submit(&files)))
}

/// Pure core of [`si_preserve_until`].
pub fn preserve_until_bytes(
    match_at: u64,
    decided: u32,
    clocks: &[u8],
) -> Result<[u8; PRESERVE_OUT_LEN], i32> {
    let decided = flag(decided)?;
    let clocks = dec(ec::decode_clocks(capped(clocks)?))?;
    let mut out = [0u8; PRESERVE_OUT_LEN];
    if let Some(t) = ev::preserve_until(match_at, decided, &clocks) {
        out[0] = 1;
        out[1..].copy_from_slice(&t.to_le_bytes());
    }
    Ok(out)
}

/// Pure core of [`si_case_apply_file_event`].
pub fn apply_file_event_bytes(
    case: &[u8],
    index: u32,
    event: &[u8],
    cap: usize,
) -> Result<Vec<u8>, i32> {
    let c = case_in(case)?;
    let e = dec(ec::decode_event(capped(event)?))?;
    case_out(ev::apply_file_event(&c, index as usize, &e), cap)
}

/// Pure core of [`si_case_confirm`].
pub fn confirm_bytes(case: &[u8], at: u64, cap: usize) -> Result<Vec<u8>, i32> {
    case_out(ev::confirm(&case_in(case)?, at), cap)
}

/// Pure core of [`si_case_clear`].
pub fn clear_bytes(case: &[u8], approval: &[u8], at: u64, cap: usize) -> Result<Vec<u8>, i32> {
    let c = case_in(case)?;
    let a = dec(ec::decode_approval(capped(approval)?))?;
    case_out(ev::clear(&c, &a, at), cap)
}

/// Pure core of [`si_case_set_hold`].
pub fn set_hold_bytes(
    case: &[u8],
    by: &[u8],
    reason: &[u8],
    at: u64,
    cap: usize,
) -> Result<Vec<u8>, i32> {
    let c = case_in(case)?;
    let by = dec(ec::decode_two(capped(by)?))?;
    case_out(ev::set_evidence_legal_hold(&c, &by, text(reason)?, at), cap)
}

/// Pure core of [`si_case_lift_hold`].
pub fn lift_hold_bytes(case: &[u8], by: &[u8], cap: usize) -> Result<Vec<u8>, i32> {
    let c = case_in(case)?;
    let by = dec(ec::decode_two(capped(by)?))?;
    case_out(ev::lift_evidence_legal_hold(&c, &by), cap)
}

/// Pure core of [`si_case_begin_destruction_on_request`].
pub fn destroy_on_request_bytes(
    case: &[u8],
    request_ref: &[u8],
    by: &[u8],
    at: u64,
    cap: usize,
) -> Result<Vec<u8>, i32> {
    let c = case_in(case)?;
    let by = dec(ec::decode_two(capped(by)?))?;
    case_out(
        ev::begin_destruction_on_request(&c, text(request_ref)?, &by, at),
        cap,
    )
}

/// Pure core of [`si_case_begin_destruction_expired`].
pub fn destroy_expired_bytes(
    case: &[u8],
    until_present: u32,
    until: u64,
    now: u64,
    shared: u32,
    cap: usize,
) -> Result<Vec<u8>, i32> {
    let c = case_in(case)?;
    let until = opt_u64(until_present, until)?;
    case_out(
        ev::begin_destruction_expired(&c, until, now, flag(shared)?),
        cap,
    )
}

/// Pure core of [`si_case_reaper_eligibility`]: 0 when eligible, else the `INELIGIBLE_*` code.
pub fn eligibility_code(
    case: &[u8],
    until_present: u32,
    until: u64,
    now: u64,
    shared: u32,
) -> Result<i32, i32> {
    let c = case_in(case)?;
    let until = opt_u64(until_present, until)?;
    match ev::reaper_eligibility(&c, until, now, flag(shared)?) {
        Ok(()) => Ok(0),
        Err(i) => Err(ineligible_code(i)),
    }
}

/// Pure core of [`si_check_release_hold`]: 0 when the release is allowed.
pub fn release_code_for(
    case: &[u8],
    target: u32,
    until_present: u32,
    until: u64,
    now: u64,
    copies_held: u32,
    by: &[u8],
) -> Result<i32, i32> {
    let c = case_in(case)?;
    let target = match target {
        0 => HoldTarget::ServingKey,
        1 => HoldTarget::EvidenceKey,
        _ => return Err(ERR_BAD_ARGUMENT),
    };
    let until = opt_u64(until_present, until)?;
    let by = dec(ec::decode_two(capped(by)?))?;
    match ev::check_release_hold(&c, target, until, now, flag(copies_held)?, &by) {
        Ok(()) => Ok(0),
        Err(r) => Err(release_code(r)),
    }
}

/// Wire tag of a case kind: 0 known hash, 1 classifier, 2 removal request, 3 moderator sighting.
pub fn kind_from_code(k: u32) -> Option<CaseKind> {
    Some(match k {
        0 => CaseKind::KnownHash,
        1 => CaseKind::Classifier,
        2 => CaseKind::RemovalRequest,
        3 => CaseKind::ModeratorSighting,
        _ => return None,
    })
}

// ---- C ABI ------------------------------------------------------------------------------------

/// Borrows an input or returns its error code from the enclosing function.
macro_rules! input {
    ($ptr:expr, $len:expr) => {
        // SAFETY: the enclosing function's contract.
        match unsafe { input_slice($ptr, $len) } {
            Ok(s) => s,
            Err(c) => return c,
        }
    };
}

fn code(r: Result<i32, i32>) -> i32 {
    match r {
        Ok(v) => v,
        Err(c) => c,
    }
}

/// The next state tag after an event (layer 3 `transition`). `state` is an evidence state tag
/// (0 pending, 1 present, 2 failed, 3 missing, 4 file-without-evidence); `event` is an encoded
/// event (tag 0 put succeeded, 1 put failed, 2 object missing, 3 decided without evidence
/// followed by two actor ids). Returns the new tag, or [`ERR_TRANSITION_REFUSED`].
///
/// # Safety
/// `event` must point to `event_len` readable bytes in this module's memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_evidence_transition(
    state: u32,
    event: *const u8,
    event_len: usize,
) -> i32 {
    let event = input!(event, event_len);
    code(transition_code(state, event))
}

/// A case's evidence state derived from its files: 0 failed, 1 pending, 2 present.
///
/// # Safety
/// `states` must point to `states_len` readable bytes (a `u32` count then one tag byte each).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_evidence(states: *const u8, states_len: usize) -> i32 {
    let states = input!(states, states_len);
    code(case_evidence_code(states))
}

/// What the drain may do with one report file: 0 send with file, 1 send without, 2 hold.
#[unsafe(no_mangle)]
pub extern "C" fn si_report_file_disposition(state: u32, object_found: u32) -> i32 {
    code(disposition_code(state, object_found))
}

/// 1 when a report may be submitted (it has files and none holds it), else 0.
///
/// # Safety
/// `files` must point to `files_len` readable bytes (a `u32` count, then a state tag byte and a
/// found flag byte per file).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_report_may_submit(files: *const u8, files_len: usize) -> i32 {
    let files = input!(files, files_len);
    code(may_submit_code(files))
}

/// One preservation period, in seconds (366 days).
#[unsafe(no_mangle)]
pub extern "C" fn si_preservation_period_secs() -> u64 {
    ev::preservation_period_secs()
}

/// The earliest time evidence may be released. Writes [`PRESERVE_OUT_LEN`] bytes at `out`: byte 0
/// is 1 when there is a time (0 means **keep everything**), then the time as a `u64`
/// little-endian. `decided` is 0 or 1. Returns [`PRESERVE_OUT_LEN`] or a negative code.
///
/// # Safety
/// `clocks` must point to `clocks_len` readable bytes and `out` to [`PRESERVE_OUT_LEN`] writable
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_preserve_until(
    match_at: u64,
    decided: u32,
    clocks: *const u8,
    clocks_len: usize,
    out: *mut u8,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let clocks = input!(clocks, clocks_len);
    match preserve_until_bytes(match_at, decided, clocks) {
        Ok(b) => {
            // SAFETY: contract above.
            unsafe { core::slice::from_raw_parts_mut(out, PRESERVE_OUT_LEN) }.copy_from_slice(&b);
            PRESERVE_OUT_LEN as i32
        }
        Err(c) => c,
    }
}

/// Applies an evidence event to file `index` of a case. Writes the new case.
///
/// # Safety
/// `case` and `event` must point to their lengths in readable bytes and `out` to `out_cap`
/// writable bytes (allocate the case length plus [`CASE_HEADROOM`]).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_apply_file_event(
    case: *const u8,
    case_len: usize,
    index: u32,
    event: *const u8,
    event_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    let event = input!(event, event_len);
    // SAFETY: contract above; the core checked the length against `out_cap`.
    unsafe { emit(out, apply_file_event_bytes(case, index, event, out_cap)) }
}

/// Records a confirmation. Writes the new case.
///
/// # Safety
/// As [`si_case_apply_file_event`], without `event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_confirm(
    case: *const u8,
    case_len: usize,
    at: u64,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    // SAFETY: contract above.
    unsafe { emit(out, confirm_bytes(case, at, out_cap)) }
}

/// Records a clear (false positive). `approval` is tag 0 plus one actor id, or tag 1 plus two
/// different ids. Writes the new case.
///
/// # Safety
/// As [`si_case_apply_file_event`], with `approval` in place of `event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_clear(
    case: *const u8,
    case_len: usize,
    approval: *const u8,
    approval_len: usize,
    at: u64,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    let approval = input!(approval, approval_len);
    // SAFETY: contract above.
    unsafe { emit(out, clear_bytes(case, approval, at, out_cap)) }
}

/// Sets an evidence legal hold. `by` is two different actor ids; `reason` is raw UTF-8 (not
/// versioned). Allocate the case length plus the reason length plus [`CASE_HEADROOM`].
///
/// # Safety
/// Each input pointer must point to its length in readable bytes and `out` to `out_cap` writable
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_set_hold(
    case: *const u8,
    case_len: usize,
    by: *const u8,
    by_len: usize,
    reason: *const u8,
    reason_len: usize,
    at: u64,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    let by = input!(by, by_len);
    let reason = input!(reason, reason_len);
    // SAFETY: contract above.
    unsafe { emit(out, set_hold_bytes(case, by, reason, at, out_cap)) }
}

/// Lifts an evidence legal hold. Two different people.
///
/// # Safety
/// As [`si_case_clear`], with `by` in place of `approval` and no `at`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_lift_hold(
    case: *const u8,
    case_len: usize,
    by: *const u8,
    by_len: usize,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    let by = input!(by, by_len);
    // SAFETY: contract above.
    unsafe { emit(out, lift_hold_bytes(case, by, out_cap)) }
}

/// Records the start of destruction on a law-enforcement request. **Records only; deletes
/// nothing.** `request_ref` is raw UTF-8.
///
/// # Safety
/// As [`si_case_set_hold`], with `request_ref` in place of `reason`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_begin_destruction_on_request(
    case: *const u8,
    case_len: usize,
    request_ref: *const u8,
    request_ref_len: usize,
    by: *const u8,
    by_len: usize,
    at: u64,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    let request_ref = input!(request_ref, request_ref_len);
    let by = input!(by, by_len);
    // SAFETY: contract above.
    unsafe {
        emit(
            out,
            destroy_on_request_bytes(case, request_ref, by, at, out_cap),
        )
    }
}

/// Records the start of destruction by the expiry reaper when eligible. `until_present` is 0 or 1
/// (0 means the clock says keep everything). A refusal returns the `INELIGIBLE_*` code.
///
/// # Safety
/// As [`si_case_confirm`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_begin_destruction_expired(
    case: *const u8,
    case_len: usize,
    until_present: u32,
    until: u64,
    now: u64,
    shares_digest: u32,
    out: *mut u8,
    out_cap: usize,
) -> i32 {
    if out.is_null() {
        return ERR_NULL_POINTER;
    }
    let case = input!(case, case_len);
    // SAFETY: contract above.
    unsafe {
        emit(
            out,
            destroy_expired_bytes(case, until_present, until, now, shares_digest, out_cap),
        )
    }
}

/// 0 when the reaper may begin destroying this case's evidence, else the `INELIGIBLE_*` code.
///
/// # Safety
/// `case` must point to `case_len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_case_reaper_eligibility(
    case: *const u8,
    case_len: usize,
    until_present: u32,
    until: u64,
    now: u64,
    shares_digest: u32,
) -> i32 {
    let case = input!(case, case_len);
    code(eligibility_code(
        case,
        until_present,
        until,
        now,
        shares_digest,
    ))
}

/// 0 when a media hold may be released, else the `RELEASE_*` code. `target` is 0 for the serving
/// key and 1 for the evidence key. `by` is two different actor ids.
///
/// # Safety
/// `case` and `by` must point to their lengths in readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn si_check_release_hold(
    case: *const u8,
    case_len: usize,
    target: u32,
    until_present: u32,
    until: u64,
    now: u64,
    evidence_copies_held: u32,
    by: *const u8,
    by_len: usize,
) -> i32 {
    let case = input!(case, case_len);
    let by = input!(by, by_len);
    code(release_code_for(
        case,
        target,
        until_present,
        until,
        now,
        evidence_copies_held,
        by,
    ))
}
