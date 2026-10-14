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
  "memory", "si_abi_version", "si_alloc", "si_backoff_secs", "si_classify_status",
  "si_deletion_window_elapsed", "si_free", "si_match_request", "si_pdq", "si_pdq_dihedral",
  "si_report_apply", "si_report_new", "si_report_next_step", "si_report_request_retract",
  "si_report_retry_failed", "si_report_sync_credentials", "si_retry_decide", "si_strongest",
  "si_within_cap",
], "unexpected export set");

const inst = new WebAssembly.Instance(module_, {}); // synchronous, nothing to import
const x = inst.exports;
assert.equal(x.si_abi_version(), 3);

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

// Layer 4: replay wasm/vectors/layer4.txt (generated from the Rust tests).
// Each call allocates its inputs and an output of input + 1024 bytes (the documented headroom).
function withBuf(inputs, outCap, fn) {
  const ptrs = inputs.map((b) => x.si_alloc(b.length));
  const outPtr = x.si_alloc(outCap);
  try {
    inputs.forEach((b, i) => new Uint8Array(x.memory.buffer, ptrs[i], b.length).set(b));
    const rc = fn(ptrs, outPtr);
    const out = rc > 0 ? new Uint8Array(x.memory.buffer, outPtr, rc).slice() : new Uint8Array(0);
    return { rc, out };
  } finally {
    inputs.forEach((b, i) => x.si_free(ptrs[i], b.length));
    x.si_free(outPtr, outCap);
  }
}
const cap4 = (state) => state.length + 1024;
let vectors4 = 0;
const l4Url = new URL("./vectors/layer4.txt", import.meta.url);
for (const line of readFileSync(l4Url, "utf8").split("\n")) {
  if (!line || line.startsWith("#")) continue;
  const f = line.split(" ");
  const kind = f[0];
  if (kind === "new") {
    const r = withBuf([], 1024, (_p, o) => x.si_report_new(+f[2], +f[3], o, 1024));
    assert.deepEqual(r.out, fromHex(f[4]), `new ${f[1]}`);
  } else if (kind === "next") {
    const st = fromHex(f[2]);
    const r = withBuf([st], 64, (p, o) => x.si_report_next_step(p[0], st.length, BigInt(f[3]), o, 64));
    assert.deepEqual(r.out, fromHex(f[4]), `next ${f[1]}`);
  } else if (kind === "apply") {
    const [st, step, ans] = [fromHex(f[2]), fromHex(f[3]), fromHex(f[4])];
    const c = cap4(st);
    const r = withBuf([st, step, ans], c, (p, o) =>
      x.si_report_apply(p[0], st.length, p[1], step.length, p[2], ans.length, BigInt(f[5]), o, c));
    assert.deepEqual(r.out, fromHex(f[6]), `apply ${f[1]}`);
  } else if (kind === "sync") {
    const st = fromHex(f[2]);
    const r = withBuf([st], cap4(st), (p, o) => x.si_report_sync_credentials(p[0], st.length, +f[3], o, cap4(st)));
    assert.deepEqual(r.out, fromHex(f[4]), `sync ${f[1]}`);
  } else if (kind === "retract" || kind === "retry") {
    const st = fromHex(f[2]);
    const fn = kind === "retract" ? x.si_report_request_retract : x.si_report_retry_failed;
    const r = withBuf([st], cap4(st), (p, o) => fn(p[0], st.length, o, cap4(st)));
    if (/^-/.test(f[3])) assert.equal(r.rc, +f[3], `${kind} ${f[1]}`);
    else assert.deepEqual(r.out, fromHex(f[3]), `${kind} ${f[1]}`);
  } else if (kind === "backoff") {
    assert.equal(x.si_backoff_secs(+f[1]), BigInt(f[2]), `backoff ${f[1]}`);
  } else if (kind === "window") {
    assert.equal(x.si_deletion_window_elapsed(BigInt(f[1]), BigInt(f[2]), BigInt(f[3]), BigInt(f[4]), BigInt(f[5])), +f[6]);
  } else {
    assert.fail(`unknown vector kind ${kind}`);
  }
  vectors4++;
}
assert.ok(vectors4 >= 20, "layer 4 vectors present");
// Every prefix of a valid state is refused with a code, never a trap.
{
  const state = fromHex(readFileSync(l4Url, "utf8").split("\n").find((l) => l.startsWith("next s0 ")).split(" ")[2]);
  for (let n = 0; n < state.length; n++) {
    const r = withBuf([state.subarray(0, n)], 64, (p, o) => x.si_report_next_step(p[0], n, 0n, o, 64));
    assert.ok(r.rc < 0, `state prefix ${n}`);
  }
}

// Memory is reusable: many round trips leave the module healthy.
const before = x.memory.buffer.byteLength;
for (let i = 0; i < 200; i++) call(x.si_pdq, new Uint8Array(64 * 64 * 3).fill(i), 64, 64, 33);
assert.ok(x.memory.buffer.byteLength <= before + 8 * 1024 * 1024, "memory growth bounded");

console.log(`wasm ok: ${checked} frames bit-exact against the reference fixture, ${vectors} layer 2 and ${vectors4} layer 4 vectors, ${exportNames.length} exports, no imports`);
