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
]


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
    print(f"\n{len(MUTANTS) - len(survived)} of {len(MUTANTS)} killed")
    if survived:
        print("survivors / not applied:", ", ".join(survived))
        sys.exit(1)


if __name__ == "__main__":
    main()
