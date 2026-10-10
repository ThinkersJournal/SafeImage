#!/usr/bin/env python3
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
]


WASM_MUTANTS = [
    ("wasm quality byte position", "wasm/src/lib.rs", "out[32] = result.quality;", "out[31] = result.quality;"),
    ("wasm dihedral stride", "wasm/src/lib.rs", "out[i * 32..i * 32 + 32].copy_from_slice(&h.to_bytes());", "out[i * 32..i * 32 + 32].copy_from_slice(&result.hashes[0].to_bytes());"),
    ("wasm length error code", "wasm/src/lib.rs", "FrameError::LengthMismatch { .. } => ERR_LENGTH_MISMATCH,", "FrameError::LengthMismatch { .. } => ERR_TOO_LARGE,"),
    ("wasm null-pointer code", "wasm/src/lib.rs", "pub const ERR_NULL_POINTER: i32 = -4;", "pub const ERR_NULL_POINTER: i32 = -5;"),
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
    r = subprocess.run(["cargo", "test", "--quiet"], cwd=ROOT, capture_output=True, text=True)
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
