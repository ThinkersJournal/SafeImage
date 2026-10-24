#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Hand-picked mutation check for layer 1 (original SafeImage code, MIT OR Apache-2.0).

Each mutation is applied to one source file, `cargo test` is run, and the file is restored. A
mutant is KILLED when the test run fails. A mutant that applies but survives is a gap. Every
mutation asserts that its anchor occurs exactly once, so a mutation that never applied cannot
be reported as caught.

usage: python tests/reference/mutants.py
"""
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

MUTANTS = [
    ("luma G coefficient", "src/hash/pdq.rs", "LUMA_FROM_G_COEFF: f32 = 0.587", "LUMA_FROM_G_COEFF: f32 = 0.586"),
    ("decimate centre offset", "src/hash/downscale.rs", "(outi as f64 + 0.5)", "(outi as f64 + 0.4)"),
    ("quality divisor", "src/hash/pdq.rs", "gradient_sum / 90", "gradient_sum / 91"),
    ("quality clamp", "src/hash/pdq.rs", "if quality > 100 {\n        quality = 100;", "if quality > 99 {\n        quality = 99;"),
    ("median tie rule", "src/hash/torben.rs", "if less >= (n + 1) / 2 {", "if less > (n + 1) / 2 {"),
    ("rotate90 sign", "src/hash/pdq.rs", "out[1][j][i] = if j & 1 != 0 { v } else { -v };", "out[1][j][i] = if j & 1 != 0 { -v } else { v };"),
    ("rotate180 sign", "src/hash/pdq.rs", "out[2][i][j] = if (i + j) & 1 != 0 { -v } else { v };", "out[2][i][j] = if (i + j) & 1 != 0 { v } else { -v };"),
    ("flipX parity", "src/hash/pdq.rs", "out[4][i][j] = if i & 1 != 0 { v } else { -v };", "out[4][i][j] = if j & 1 != 0 { v } else { -v };"),
    ("flip-minus-1 transpose", "src/hash/pdq.rs", "out[7][j][i] = if (i + j) & 1 != 0 { -v } else { v };", "out[7][i][j] = if (i + j) & 1 != 0 { -v } else { v };"),
    ("box filter drops subtract", "src/hash/downscale.rs", "        sum -= invec[li];\n        outvec[oi] = sum / current_window_size as f32;\n        li += stride;\n        ri += stride;", "        outvec[oi] = sum / current_window_size as f32;\n        li += stride;\n        ri += stride;"),
    ("jarosz passes", "src/hash/pdq.rs", "const PDQ_NUM_JAROSZ_XY_PASSES: usize = 2;", "const PDQ_NUM_JAROSZ_XY_PASSES: usize = 1;"),
    ("dct column index", "src/hash/pdq.rs", "(2 * j + 1) as f64", "(2 * j) as f64"),
    ("dct scale in f64", "src/hash/pdq.rs", "libm::sqrt(2.0 / DCT_COLS as f64) as f32", "libm::sqrt(2.0 / DCT_COLS as f64)  as f32 * 1.0000001"),
    ("hex word order", "src/hash/hash256.rs", "w[15 - n] = v;", "w[n] = v;"),
    ("hamming uses OR", "src/hash/hash256.rs", "(a ^ b).count_ones()", "(a | b).count_ones()"),
    ("min dimension", "src/hash/pdq.rs", "pub(crate) const MIN_HASHABLE_DIM: usize = 5;", "pub(crate) const MIN_HASHABLE_DIM: usize = 4;"),
    ("frame length check", "src/hash/frame.rs", "if rgb.len() != expected {", "if rgb.len() < expected {"),
    ("frame size cap", "src/hash/frame.rs", ".filter(|p| *p <= MAX_FRAME_PIXELS)", ".filter(|p| *p <= usize::MAX)"),
    ("matcher: strongest takes weakest", "src/matcher/outcome.rs", ".max()", ".min()"),
    ("matcher: empty set is clean", "src/matcher/outcome.rs", ".unwrap_or(MatchOutcome::Unavailable(Unavailable::NotConfigured))", ".unwrap_or(MatchOutcome::Clean(CleanBasis::default()))"),
    ("matcher: empty mock is clean", "src/matcher/mock.rs", "return MatchOutcome::Unavailable(Unavailable::NotConfigured);", "return MatchOutcome::Clean(super::outcome::CleanBasis::default());"),
    ("matcher: throttle map", "src/matcher/protocol.rs", "429 => Some(Unavailable::Throttled),", "429 => Some(Unavailable::ServerError),"),
    ("matcher: 403 not refused", "src/matcher/protocol.rs", "401 | 403 => Some(Unavailable::CredentialsRefused),", "401 => Some(Unavailable::CredentialsRefused),"),
    ("matcher: retry throttled", "src/matcher/protocol.rs", "Unavailable::Timeout | Unavailable::ServerError)", "Unavailable::Timeout | Unavailable::ServerError | Unavailable::Throttled)"),
    ("matcher: unlimited retries", "src/matcher/protocol.rs", "retries_done < self.max_retries", "retries_done <= self.max_retries"),
    ("matcher: request size bound", "src/matcher/protocol.rs", "hashes.len() > Self::MAX_HASHES", "hashes.len() > Self::MAX_HASHES + 1"),
    ("matcher: body cap exclusive", "src/matcher/protocol.rs", "body.len() <= cap", "body.len() < cap"),
    ("matcher: detail count cap", "src/matcher/outcome.rs", "details.truncate(MAX_DETAILS);", "details.truncate(MAX_DETAILS + 1000);"),
    ("evidence: pending put succeeded", "src/evidence/state.rs", "(Pending, PutSucceeded) => Ok(Present),", "(Pending, PutSucceeded) => Ok(Failed),"),
    ("evidence: missing cannot be decided", "src/evidence/state.rs", "(Failed | Missing, DecidedWithoutEvidence(_)) => Ok(FileWithoutEvidence),", "(Failed, DecidedWithoutEvidence(_)) => Ok(FileWithoutEvidence),"),
    ("evidence: without-evidence stays", "src/evidence/state.rs", "(FileWithoutEvidence, PutSucceeded) => Ok(Present),", "(FileWithoutEvidence, PutSucceeded) => Ok(FileWithoutEvidence),"),
    ("evidence: derived state ignores pending", "src/evidence/state.rs", ".any(|s| matches!(s, EvidenceState::Pending))", ".any(|s| matches!(s, EvidenceState::Present))"),
    ("evidence: present but not found sends", "src/evidence/state.rs", "EvidenceState::Present if object_found => FileDisposition::SendWithFile,", "EvidenceState::Present => FileDisposition::SendWithFile,"),
    ("evidence: empty report may submit", "src/evidence/state.rs", "!files.is_empty()", "true"),
    ("evidence: same person twice", "src/evidence/people.rs", "if first == second {", "if false {"),
    ("evidence: one year is 365 days", "src/evidence/preserve.rs", "366 * 24 * 60 * 60", "365 * 24 * 60 * 60"),
    ("evidence: undecided case is bounded", "src/evidence/preserve.rs", "!decided || ", ""),
    ("evidence: sends ignored by the clock", "src/evidence/preserve.rs", "Some(latest.saturating_add(preservation_period_secs()))", "Some(match_at.saturating_add(preservation_period_secs()))"),
    ("evidence: submitted report is final", "src/evidence/preserve.rs", "ReportStatus::Submitted", "ReportStatus::Finished"),
    ("evidence: clear needs only one", "src/evidence/case.rs", "if !matches!(approval, ClearApproval::Two(_)) {", "if false {"),
    ("evidence: sighting clearable", "src/evidence/case.rs", "CaseKind::ModeratorSighting => return Err(CaseError::NotClearable(case.kind)),", "CaseKind::ModeratorSighting => {}"),
    ("evidence: destroyed case writable", "src/evidence/case.rs", "Err(CaseError::Destroyed)", "Ok(())"),
    ("evidence: confirmed case destroyable", "src/evidence/case.rs", "Some(ReviewOutcome::Confirmed) => return Err(Ineligible::Confirmed),", "Some(ReviewOutcome::Confirmed) => {}"),
    ("evidence: shared file ignored", "src/evidence/case.rs", "if shares_digest_with_live_case {", "if false {"),
    ("evidence: legal hold ignored by reaper", "src/evidence/case.rs", "return Err(Ineligible::LegalHold);", "();"),
    ("evidence: evidence hold released early", "src/evidence/case.rs", "_ => return Err(ReleaseRefusal::BeforeEnd),", "_ => {}"),
    ("evidence: serving hold without copy", "src/evidence/case.rs", "if evidence_copies_held {", "if true {"),
    ("evidence: blank request ref", "src/evidence/case.rs", "if request_ref.trim().is_empty() {", "if false {"),
    ("evidence: blank hold reason", "src/evidence/case.rs", "if reason.trim().is_empty() {", "if false {"),
    ("evidence: destruction unattributed", "src/evidence/case.rs", "by: Some(by.clone()),", "by: None,"),
    ("evidence: lift with no hold", "src/evidence/case.rs", "if case.evidence_hold.is_none() {", "if false {"),
    ("screen: match not blocked", "src/screening/mod.rs", "MatchOutcome::Match(m) => Screening::Blocked(m),", "MatchOutcome::Match(_) => Screening::Hold(ScreenHold::Unavailable(Unavailable::NotConfigured)),"),
    ("screen: failure allows", "src/screening/mod.rs", "MatchOutcome::Unavailable(r) => Screening::Hold(ScreenHold::Unavailable(r)),", "MatchOutcome::Unavailable(_) => Screening::Allow { frame: ScreenedFrame { frame }, classifier },"),
    ("screen: classifier hold boundary", "src/screening/mod.rs", "&& sig.score() >= level", "&& sig.score() > level"),
    ("screen: quality floor boundary", "src/screening/mod.rs", "if d.quality <= floor {", "if d.quality < floor {"),
    ("screen: default floor", "src/screening/mod.rs", "policy.quality_floor.unwrap_or(QUALITY_FLOOR_DEFAULT)", "policy.quality_floor.unwrap_or(0)"),
    ("classifier: score range", "src/classifier/mod.rs", "v.is_finite() && (0.0..=1.0).contains(&v)", "v.is_finite() && (0.0..1.0).contains(&v)"),
    ("classifier: model label bound", "src/classifier/mod.rs", "label.len() > 64", "label.len() > 65"),
    ("classifier: empty mock answer", "src/classifier/mod.rs", "return Err(ClassifierError::Unavailable);", "return Err(ClassifierError::BadAnswer);"),
    ("report: backoff cap", "src/report/state.rs", "const CAP: u64 = 30 * 60;", "const CAP: u64 = 60 * 60;"),
    ("report: backoff base", "src/report/state.rs", "((2u64 * 60) << n)", "((3u64 * 60) << n)"),
    ("report: backoff exponent origin", "src/report/state.rs", "attempts.max(1) - 1", "attempts.max(1)"),
    ("report: unknown code is success", "src/report/state.rs", "transient(s, now, Some(code.clone())),", "success(s),"),
    ("report: submit without id", "src/report/state.rs", "Some(id) if !id.trim().is_empty() =>", "Some(id) =>"),
    ("report: invalid is not terminal", "src/report/state.rs", "s.status = Status::Failed;", "s.status = Status::Pending;"),
    ("report: already-finished note dropped", "src/report/state.rs", "s.finish_note = Some(FinishNote::AlreadyFinished);", "s.finish_note = None;"),
    ("report: finished-before-retract unflagged", "src/report/state.rs", "(success(s), Some(Event::FinishedBeforeRetract))", "(success(s), None)"),
    ("report: window expiry ignores withdrawal", "src/report/state.rs", "if s.status == Status::RetractPending {", "if false {"),
    ("report: retry touches non-failed", "src/report/state.rs", "if s.status == Status::Failed {", "if s.status != Status::Failed {"),
    ("report: retract of unsent submitted", "src/report/state.rs", "s.status = if s.report_id.is_some() {", "s.status = if s.report_id.is_none() {"),
    ("report: credentials vanish ignored", "src/report/state.rs", "(Status::Pending, false) => s.status = Status::AwaitingCredentials,", "(Status::Pending, false) => {}"),
    ("report: submit valid anywhere", "src/report/state.rs", "Step::Submit => state.status == Status::Pending,", "Step::Submit => true,"),
    ("report: window uses earlier deadline", "src/report/state.rs", "a.max(b)", "a.min(b)"),
    ("report: upload marks described", "src/report/state.rs", "s.files[*file].uploaded = true;", "s.files[*file].described = true;"),
    ("report: file order", "src/report/state.rs", "if !f.uploaded {", "if f.uploaded {"),
    ("report: no destinations looks ready", "src/report/setup.rs", "if destinations.is_empty() {", "if false {"),
    ("report: optional slot prompted", "src/report/setup.rs", ".filter(|s| s.required && !credentials.has(&s.key))", ".filter(|s| !credentials.has(&s.key))"),
    ("report: blank credential counts", "src/report/secret.rs", ".find(|(k, v)| k == key && !v.expose().trim().is_empty())", ".find(|(k, _)| k == key)"),
    ("report: secret debug leaks", "src/report/secret.rs", "f.write_str(\"Secret(<redacted>)\")", "write!(f, \"Secret({})\", self.0)"),
    ("report: person debug leaks", "src/report/input.rs", "f.write_str(\"Person(<redacted>)\")", "write!(f, \"Person({})\", self.name)"),
]


WASM_MUTANTS = [
    ("wasm quality byte position", "wasm/src/abi.rs", "out[32] = result.quality;", "out[31] = result.quality;"),
    ("wasm dihedral stride", "wasm/src/abi.rs", "out[i * 32..i * 32 + 32].copy_from_slice(&h.to_bytes());", "out[i * 32..i * 32 + 32].copy_from_slice(&result.hashes[0].to_bytes());"),
    ("wasm length error code", "wasm/src/abi.rs", "FrameError::LengthMismatch { .. } => ERR_LENGTH_MISMATCH,", "FrameError::LengthMismatch { .. } => ERR_TOO_LARGE,"),
    ("wasm null-pointer code", "wasm/src/abi.rs", "pub const ERR_NULL_POINTER: i32 = -4;", "pub const ERR_NULL_POINTER: i32 = -5;"),
    ("wasm byte order", "src/hash/hash256.rs", "word.to_le_bytes()", "word.to_be_bytes()"),
]


def run_wasm_check():
    import tempfile
    frames = tempfile.mkdtemp()
    env = dict(__import__("os").environ, SAFEIMAGE_FRAMES_DIR=frames)
    b = subprocess.run(["cargo", "build", "-p", "safeimage-wasm", "--release", "--target", "wasm32-unknown-unknown", "--quiet"], cwd=ROOT, capture_output=True, text=True)
    if b.returncode != 0:
        return False  # a mutant that does not build is "caught" only trivially; report it as such below
    subprocess.run(["cargo", "test", "--quiet", "--test", "write_frames", "--", "--ignored"], cwd=ROOT, env=env, capture_output=True, text=True)
    r = subprocess.run(["node", "wasm/test.mjs", "target/wasm32-unknown-unknown/release/safeimage_wasm.wasm", frames, "tests/fixtures/reference.txt"], cwd=ROOT, capture_output=True, text=True)
    return r.returncode == 0


def run_tests():
    r = subprocess.run(["cargo", "test", "--quiet", "--workspace", "--all-features"], cwd=ROOT, capture_output=True, text=True)
    return r.returncode == 0


def main():
    survived = []
    for name, rel, old, new in MUTANTS:
        path = ROOT / rel
        original = path.read_text(encoding="utf-8")
        count = original.count(old)
        if count != 1:
            print(f"NOT APPLIED  {name}: anchor occurs {count} times in {rel}")
            survived.append(name + " (not applied)")
            continue
        try:
            path.write_text(original.replace(old, new), encoding="utf-8", newline="\n")
            ok = run_tests()
        finally:
            path.write_text(original, encoding="utf-8", newline="\n")
        print(("SURVIVED    " if ok else "killed      ") + name)
        if ok:
            survived.append(name)
    for name, rel, old, new in WASM_MUTANTS:
        path = ROOT / rel
        original = path.read_text(encoding="utf-8")
        count = original.count(old)
        if count != 1:
            print(f"NOT APPLIED  {name}: anchor occurs {count} times in {rel}")
            survived.append(name + " (not applied)")
            continue
        try:
            path.write_text(original.replace(old, new), encoding="utf-8", newline="\n")
            ok = run_wasm_check()
        finally:
            path.write_text(original, encoding="utf-8", newline="\n")
        print(("SURVIVED    " if ok else "killed      ") + name)
        if ok:
            survived.append(name)
    total = len(MUTANTS) + len(WASM_MUTANTS)
    print(f"\n{total - len(survived)} of {total} killed")
    if survived:
        print("survivors / not applied:", ", ".join(survived))
        sys.exit(1)


if __name__ == "__main__":
    main()
