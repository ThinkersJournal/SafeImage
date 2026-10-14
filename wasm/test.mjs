// Checks the built module against the upstream reference fixtures, the way a worker host would
// use it: compiled module, synchronous instantiation, no imports, bytes in and out.
//
// usage: node wasm/test.mjs <path to .wasm> <frames dir> <tests/fixtures/reference.txt>
// The frames dir is written by `SAFEIMAGE_FRAMES_DIR=<dir> cargo test --test write_frames -- --ignored`.
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";

const [wasmPath, framesDir, refPath] = process.argv.slice(2);
const module_ = new WebAssembly.Module(readFileSync(wasmPath));

// Acceptance criteria from the consumer's workerd gate.
assert.deepEqual(WebAssembly.Module.imports(module_), [], "module must have no imports");
const exportNames = WebAssembly.Module.exports(module_).map((e) => e.name).sort();
assert.deepEqual(exportNames, [
  "memory", "si_abi_version", "si_alloc", "si_classify_status", "si_free", "si_match_request",
  "si_pdq", "si_pdq_dihedral", "si_retry_decide", "si_strongest", "si_within_cap",
], "unexpected export set");

const inst = new WebAssembly.Instance(module_, {}); // synchronous, nothing to import
const x = inst.exports;
assert.equal(x.si_abi_version(), 2);

function call(fn, rgb, w, h, outLen) {
  const inPtr = x.si_alloc(rgb.length);
  const outPtr = x.si_alloc(outLen);
  try {
    new Uint8Array(x.memory.buffer, inPtr, rgb.length).set(rgb);
    const rc = fn(inPtr, rgb.length, w, h, outPtr);
    const out = new Uint8Array(x.memory.buffer, outPtr, outLen).slice();
    return { rc, out };
  } finally {
    x.si_free(inPtr, rgb.length);
    x.si_free(outPtr, outLen);
  }
}

// Upstream text form: 64 hex digits, word 15 first. The binding writes word 0 first, LE.
function hex(bytes32) {
  let s = "";
  for (let i = 15; i >= 0; i--) {
    const w = bytes32[i * 2] | (bytes32[i * 2 + 1] << 8);
    s += w.toString(16).padStart(4, "0");
  }
  return s;
}

// Parse the reference fixture.
const ref = new Map();
let cur = null;
for (const line of readFileSync(refPath, "utf8").split("\n")) {
  const f = line.split(" ");
  if (f[0] === "frame") { cur = { name: f[1], w: +f[2], h: +f[3], dihedral: [] }; ref.set(cur.name, cur); }
  else if (f[0] === "pdq" && cur) { cur.pdq = f[1]; cur.q = +f[2]; }
  else if (f[0] === "dihedral_quality" && cur) cur.dq = +f[1];
  else if (f[0] === "dihedral" && cur) cur.dihedral[+f[1]] = f[2];
}
assert.ok(ref.size >= 10, "fixture has frames");

let checked = 0;
for (const r of ref.values()) {
  const rgb = new Uint8Array(readFileSync(`${framesDir}/${r.name}.rgb`));
  const a = call(x.si_pdq, rgb, r.w, r.h, 33);
  assert.equal(a.rc, 0, `${r.name}: rc`);
  assert.equal(hex(a.out.subarray(0, 32)), r.pdq, `${r.name}: hash`);
  assert.equal(a.out[32], r.q, `${r.name}: quality`);
  const d = call(x.si_pdq_dihedral, rgb, r.w, r.h, 257);
  assert.equal(d.rc, 0, `${r.name}: dihedral rc`);
  for (let i = 0; i < 8; i++) {
    assert.equal(hex(d.out.subarray(i * 32, i * 32 + 32)), r.dihedral[i], `${r.name}: dihedral ${i}`);
  }
  assert.equal(d.out[256], r.dq, `${r.name}: dihedral quality`);
  checked++;
}

// Typed errors, never a trap.
const six = new Uint8Array(9 * 3);
assert.equal(call(x.si_pdq, six, 0, 3, 33).rc, -1, "zero dimension");
assert.equal(call(x.si_pdq, six, 3, 4, 33).rc, -3, "length mismatch");
assert.equal(call(x.si_pdq, new Uint8Array(0), 65535, 65535, 33).rc, -2, "too large");
assert.equal(x.si_pdq(0, 5, 1, 1, 0), -4, "null out pointer");

// Layer 2: replay the vectors generated from the Rust tests (wasm/vectors/layer2.txt).
function fromHex(h) {
  const b = new Uint8Array(h.length / 2);
  for (let i = 0; i < b.length; i++) b[i] = parseInt(h.slice(i * 2, i * 2 + 2), 16);
  return b;
}
function strongest(input, cap) {
  const inPtr = x.si_alloc(input.length);
  const outPtr = x.si_alloc(cap);
  try {
    new Uint8Array(x.memory.buffer, inPtr, input.length).set(input);
    const rc = x.si_strongest(inPtr, input.length, outPtr, cap);
    const out = rc > 0 ? new Uint8Array(x.memory.buffer, outPtr, rc).slice() : new Uint8Array(0);
    return { rc, out };
  } finally {
    x.si_free(inPtr, input.length);
    x.si_free(outPtr, cap);
  }
}
let vectors = 0;
const vectorsUrl = new URL("./vectors/layer2.txt", import.meta.url);
for (const line of readFileSync(vectorsUrl, "utf8").split("\n")) {
  if (!line || line.startsWith("#")) continue;
  const [kind, name, inHex, want] = line.split(" ");
  const input = fromHex(inHex);
  const r = strongest(input, input.length + 16);
  if (kind === "strongest") {
    assert.ok(r.rc > 0, `${name}: rc ${r.rc}`);
    assert.ok(r.rc <= input.length, `${name}: output no longer than input`);
    assert.deepEqual(r.out, fromHex(want), `${name}: output`);
  } else {
    assert.equal(r.rc, +want, `${name}: error code`);
  }
  vectors++;
}
assert.ok(vectors >= 10, "layer 2 vectors present");

// Every prefix of a valid input is refused with a code, never a trap.
{
  const all = fromHex(readFileSync(vectorsUrl, "utf8").split("\n").find((l) => l.startsWith("strongest all ")).split(" ")[2]);
  for (let n = 0; n < all.length; n++) assert.ok(strongest(all.subarray(0, n), 4096).rc < 0, `prefix ${n}`);
}

// Status, cap and retry.
assert.equal(x.si_classify_status(200), 0);
assert.equal(x.si_classify_status(429), 5);
assert.equal(x.si_classify_status(70000), 7, "not a u16 is malformed");
assert.equal(x.si_within_cap(10, 10), 1);
assert.equal(x.si_within_cap(11, 10), 0);
{
  const outPtr = x.si_alloc(9);
  try {
    const rc = x.si_retry_decide(1, 250n, 1, 2, 100n, 1000n, outPtr);
    const o = new Uint8Array(x.memory.buffer, outPtr, 9).slice();
    assert.equal(rc, 9);
    assert.equal(o[0], 1);
    assert.equal(new DataView(o.buffer).getBigUint64(1, true), 250n);
    x.si_retry_decide(1, 250n, 1, 4, 100n, 1000n, outPtr); // throttled: stop
    assert.equal(new Uint8Array(x.memory.buffer, outPtr, 9)[0], 0);
  } finally {
    x.si_free(outPtr, 9);
  }
}

// Memory is reusable: many round trips leave the module healthy.
const before = x.memory.buffer.byteLength;
for (let i = 0; i < 200; i++) call(x.si_pdq, new Uint8Array(64 * 64 * 3).fill(i), 64, 64, 33);
assert.ok(x.memory.buffer.byteLength <= before + 8 * 1024 * 1024, "memory growth bounded");

console.log(`wasm ok: ${checked} frames bit-exact against the reference fixture, ${vectors} layer 2 vectors, ${exportNames.length} exports, no imports`);
