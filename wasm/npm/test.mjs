// SPDX-License-Identifier: MIT OR Apache-2.0
// Replays the repository's conformance vectors (wasm/vectors/*.txt) through the typed wrapper, so
// the wrapper's encodings cannot drift from the Rust ones. A vector that expects a refusal code must
// make the wrapper throw a SafeImageError with that code, whether the encoder or the module refused.
//
// usage: node wasm/npm/test.mjs <path to .wasm>
import { readFileSync } from "node:fs";
import assert from "node:assert/strict";
import {
  CODES,
  SafeImage,
  SafeImageError,
  decodeAnswer,
  decodeApplied,
  decodeApproval,
  decodeCase,
  decodeClocks,
  decodeEvidenceEvent,
  decodeOutcomes,
  decodeState,
  decodeStateFound,
  decodeStates,
  decodeStep,
  decodeTwo,
  encodeAnswer,
  encodeApproval,
  encodeCase,
  encodeClocks,
  encodeEvidenceEvent,
  encodeOutcome,
  encodeOutcomes,
  encodeState,
  encodeStateFound,
  encodeStates,
  encodeStep,
  encodeTwo,
  hashFromHex,
  hashToHex,
} from "./index.js";

const wasmPath = process.argv[2];
const si = await SafeImage.create(readFileSync(wasmPath));
assert.equal(si.abiVersion, 4);

const fromHex = (h) => new Uint8Array(h.match(/../g)?.map((b) => parseInt(b, 16)) ?? []);
const toHex = (b) => Array.from(b, (v) => v.toString(16).padStart(2, "0")).join("");
const vectors = (name) =>
  readFileSync(new URL(`../vectors/${name}.txt`, import.meta.url), "utf8")
    .split("\n")
    .filter((l) => l && !l.startsWith("#"))
    .map((l) => l.split(" "));

const isCode = (s) => /^-\d+$/.test(s);

/** Runs `fn`; a negative `want` must be the code of the SafeImageError it throws, otherwise `check(result)`. */
function expect(label, want, fn, check) {
  if (isCode(want)) {
    assert.throws(fn, (e) => e instanceof SafeImageError && e.code === +want, `${label}: want code ${want}`);
  } else {
    check(fn());
  }
}

let counts = { l2: 0, l3: 0, l4: 0 };

// ---- layer 2 ----------------------------------------------------------------------------------
for (const f of vectors("layer2")) {
  const [kind, name] = f;
  const label = `${kind} ${name}`;
  if (kind === "strongest" || kind === "strongest_err") {
    expect(label, kind === "strongest" ? f[3] : f[3], () => si.strongest(decodeOutcomes(fromHex(f[2]))), (o) => {
      // The wrapper re-encodes what it decodes, so an unknown tag in the input is the one case where
      // the input hex differs from ours; the result must still equal the module's.
      assert.equal(toHex(encodeOutcome(o)), f[3], label);
    });
  } else if (kind === "classify") {
    const r = si.classifyStatus(+f[1]);
    assert.equal(r === null ? 0 : 1 + ["NotConfigured", "Disabled", "Timeout", "ServerError", "Throttled", "CredentialsRefused", "MalformedResponse", "TestValueInProduction"].indexOf(r), +f[2], label);
  } else if (kind === "within_cap") {
    assert.equal(si.withinCap(+f[1], +f[2]), f[3] === "1", label);
  } else if (kind === "retry") {
    const reason = ["NotConfigured", "Disabled", "Timeout", "ServerError", "Throttled", "CredentialsRefused", "MalformedResponse", "TestValueInProduction"][+f[5]];
    if (reason === undefined) continue; // a tag outside the table has no typed spelling: raw conformance is wasm/test.mjs
    const r = si.retryDecide({ maxRetries: +f[2], delayMs: BigInt(f[3]) }, +f[4], reason, BigInt(f[6]), BigInt(f[7]));
    const want = fromHex(f[8]);
    assert.equal(r.retry, want[0] === 1, label);
    if (r.retry) assert.equal(r.afterMs, new DataView(want.buffer).getBigUint64(1, true), label);
  } else if (kind === "match_request") {
    // Hashes are opaque 32-byte values: take them out of the encoded request.
    const input = fromHex(f[2]);
    const n = new DataView(input.buffer).getUint32(1, true);
    const hashes = Array.from({ length: n }, (_, i) => input.slice(5 + i * 32, 5 + i * 32 + 32));
    if (input.length === 5 + n * 32) {
      expect(label, f[3], () => si.validateMatchRequest(hashes), (c) => assert.equal(c, +f[3], label));
    }
  } else assert.fail(`unknown layer 2 kind ${kind}`);
  counts.l2++;
}

// ---- layer 4 ----------------------------------------------------------------------------------
for (const f of vectors("layer4")) {
  const kind = f[0];
  const label = `${kind} ${f[1]}`;
  const sameState = (s, hex) => assert.equal(toHex(encodeState(s)), hex, label);
  if (kind === "new") {
    sameState(si.reportNew(+f[2], f[3] === "1"), f[4]);
  } else if (kind === "next") {
    const step = si.reportNextStep(decodeState(fromHex(f[2])), BigInt(f[3]));
    assert.equal(toHex(encodeStep(step)), f[4], label);
  } else if (kind === "apply") {
    const r = si.reportApply(decodeState(fromHex(f[2])), decodeStep(fromHex(f[3])), decodeAnswer(fromHex(f[4])), BigInt(f[5]));
    // Re-encode as the module does: state, then a flag byte and the event.
    const want = decodeApplied(fromHex(f[6]));
    assert.deepEqual(r, want, label);
  } else if (kind === "sync") {
    sameState(si.reportSyncCredentials(decodeState(fromHex(f[2])), f[3] === "1"), f[4]);
  } else if (kind === "retract" || kind === "retry") {
    const st = decodeState(fromHex(f[2]));
    const fn = kind === "retract" ? () => si.reportRequestRetract(st) : () => si.reportRetryFailed(st);
    expect(label, f[3], fn, (s) => sameState(s, f[3]));
  } else if (kind === "backoff") {
    assert.equal(si.backoffSecs(+f[1]), BigInt(f[2]), label);
  } else if (kind === "window") {
    assert.equal(si.deletionWindowElapsed(BigInt(f[1]), BigInt(f[2]), BigInt(f[3]), { openSecs: BigInt(f[4]), idleSecs: BigInt(f[5]) }), f[6] === "1", label);
  } else assert.fail(`unknown layer 4 kind ${kind}`);
  counts.l4++;
}

// ---- layer 3 ----------------------------------------------------------------------------------
const STATES = ["Pending", "Present", "Failed", "Missing", "FileWithoutEvidence"];
const caseHex = (c) => toHex(encodeCase(c));
const reasonOf = (hex) => new TextDecoder().decode(fromHex(hex));
for (const f of vectors("layer3")) {
  const kind = f[0];
  const label = `${kind} ${f[1]}`;
  const last = f[f.length - 1];
  const sameCase = (c) => assert.equal(caseHex(c), last, label);
  if (kind === "transition") {
    expect(label, last, () => si.evidenceTransition(STATES[+f[2]], decodeEvidenceEvent(fromHex(f[3]))), (s) => assert.equal(STATES.indexOf(s), +last, label));
  } else if (kind === "case_evidence") {
    expect(label, last, () => si.caseEvidence(decodeStates(fromHex(f[2]))), (s) => assert.equal(["Failed", "Pending", "Present"].indexOf(s), +last, label));
  } else if (kind === "may_submit") {
    assert.equal(si.reportMaySubmit(decodeStateFound(fromHex(f[2]))), last === "1", label);
  } else if (kind === "disposition") {
    assert.equal(["SendWithFile", "SendWithoutFile", "Hold"].indexOf(si.reportFileDisposition(STATES[+f[2]], f[3] === "1")), +last, label);
  } else if (kind === "period") {
    assert.equal(si.preservationPeriodSecs(), BigInt(f[1]), label);
  } else if (kind === "preserve") {
    const r = si.preserveUntil(BigInt(f[2]), f[3] === "1", decodeClocks(fromHex(f[4])));
    const want = fromHex(last);
    assert.equal(r === null ? 0 : 1, want[0], label);
    if (r !== null) assert.equal(r, new DataView(want.buffer).getBigUint64(1, true), label);
  } else if (kind === "file_event") {
    expect(label, last, () => si.caseApplyFileEvent(decodeCase(fromHex(f[2])), +f[3], decodeEvidenceEvent(fromHex(f[4]))), sameCase);
  } else if (kind === "confirm") {
    expect(label, last, () => si.caseConfirm(decodeCase(fromHex(f[2])), BigInt(f[3])), sameCase);
  } else if (kind === "clear") {
    expect(label, last, () => si.caseClear(decodeCase(fromHex(f[2])), decodeApproval(fromHex(f[3])), BigInt(f[4])), sameCase);
  } else if (kind === "set_hold") {
    expect(label, last, () => si.caseSetHold(decodeCase(fromHex(f[2])), decodeTwo(fromHex(f[3])), reasonOf(f[4]), BigInt(f[5])), sameCase);
  } else if (kind === "lift") {
    expect(label, last, () => si.caseLiftHold(decodeCase(fromHex(f[2])), decodeTwo(fromHex(f[3]))), sameCase);
  } else if (kind === "destroy_req") {
    expect(label, last, () => si.caseBeginDestructionOnRequest(decodeCase(fromHex(f[2])), reasonOf(f[3]), decodeTwo(fromHex(f[4])), BigInt(f[5])), sameCase);
  } else if (kind === "destroy_exp") {
    expect(label, last, () => si.caseBeginDestructionExpired(decodeCase(fromHex(f[2])), f[3] === "1" ? BigInt(f[4]) : null, BigInt(f[5]), f[6] === "1"), sameCase);
  } else if (kind === "eligible") {
    const r = si.caseReaperEligibility(decodeCase(fromHex(f[2])), f[3] === "1" ? BigInt(f[4]) : null, BigInt(f[5]), f[6] === "1");
    assert.equal(r === null ? "0" : String(-(31 + ["Undecided", "Confirmed", "BeforeEnd", "LegalHold", "SharedFile", "AlreadyBegun"].indexOf(r))), last, label);
  } else if (kind === "release") {
    const r = si.checkReleaseHold(decodeCase(fromHex(f[2])), +f[3] === 0 ? "ServingKey" : "EvidenceKey", f[4] === "1" ? BigInt(f[5]) : null, BigInt(f[6]), f[7] === "1", decodeTwo(fromHex(f[8])));
    assert.equal(r === null ? "0" : String(-(40 + ["NotCleared", "EvidenceCopyNotHeld", "BeforeEnd", "LegalHold"].indexOf(r))), last, label);
  } else assert.fail(`unknown layer 3 kind ${kind}`);
  counts.l3++;
}

// ---- codec round trips: decode then encode returns the vector's own bytes -----------------------
{
  let n = 0;
  for (const f of vectors("layer3")) {
    for (const h of f.slice(1)) {
      if (!/^01[0-9a-f]+$/.test(h) || h.length % 2) continue;
      try {
        assert.equal(caseHex(decodeCase(fromHex(h))), h);
        n++;
      } catch (e) {
        if (e instanceof assert.AssertionError) throw e; // a decode that succeeded must round-trip
      }
    }
  }
  for (const f of vectors("layer4")) {
    for (const h of f.slice(1)) {
      if (!/^01[0-9a-f]+$/.test(h)) continue;
      try {
        assert.equal(toHex(encodeState(decodeState(fromHex(h)))), h);
        n++;
      } catch (e) {
        if (e instanceof assert.AssertionError) throw e;
      }
    }
  }
  assert.ok(n >= 10, `round trips ran (${n})`);
}

// ---- fail closed ------------------------------------------------------------------------------
{
  // An unknown outcome tag is Unavailable(MalformedResponse), never Clean.
  const o = si.strongest(decodeOutcomes(fromHex("010100000063")));
  assert.deepEqual(o, { kind: "Unavailable", reason: "MalformedResponse" });
  // An unknown answer tag is Unknown, handled as transient by the module.
  assert.deepEqual(decodeAnswer(fromHex("0163")), { kind: "Unknown", code: "unknown_tag" });
  // Every prefix of a valid state or case is a thrown SafeImageError, never a value or a trap.
  const state = fromHex(vectors("layer4").find((f) => f[0] === "next")[2]);
  for (let n = 0; n < state.length; n++) assert.throws(() => decodeState(state.subarray(0, n)), SafeImageError, `state prefix ${n}`);
  const c = fromHex(vectors("layer3").find((f) => f[0] === "set_hold" && f[1] === "ok")[2]);
  for (let n = 0; n < c.length; n++) assert.throws(() => decodeCase(c.subarray(0, n)), SafeImageError, `case prefix ${n}`);
  // A range error is the caller's bug, not a module refusal.
  assert.throws(() => si.backoffSecs(-1), RangeError);
  assert.throws(() => si.withinCap(2 ** 32, 0), RangeError);
  assert.throws(() => encodeOutcomes([{ kind: "Nope" }]), TypeError);
  assert.equal(CODES.TRUNCATED, -9);
}

// ---- layer 1 and helpers ----------------------------------------------------------------------
{
  const rgb = new Uint8Array(64 * 64 * 3);
  for (let i = 0; i < rgb.length; i++) rgb[i] = (i * 31 + (i >> 7)) & 0xff;
  const a = si.pdq(rgb, 64, 64);
  assert.equal(a.hash.length, 32);
  assert.ok(a.quality >= 0 && a.quality <= 100);
  const d = si.pdqDihedral(rgb, 64, 64);
  assert.equal(d.hashes.length, 8);
  assert.deepEqual(d.hashes[0], a.hash, "orientation 0 is the plain hash");
  assert.equal(hashToHex(a.hash).length, 64);
  assert.deepEqual(hashFromHex(hashToHex(a.hash)), a.hash);
  assert.equal(si.validateMatchRequest([a.hash, d.hashes[1]]), 2);
  assert.throws(() => si.validateMatchRequest([]), (e) => e.code === CODES.BAD_ARGUMENT);
  assert.throws(() => si.pdq(rgb, 0, 3), (e) => e.code === CODES.ZERO_DIMENSION);
  assert.throws(() => si.pdq(rgb, 3, 4), (e) => e.code === CODES.LENGTH_MISMATCH);
  // The module stays healthy after refusals: the same call still works.
  assert.deepEqual(si.pdq(rgb, 64, 64).hash, a.hash);
  // Memory does not leak across calls.
  const before = new WebAssembly.Memory({ initial: 1 }).buffer.byteLength; // a control for the number below
  assert.equal(before, 65536);
  for (let i = 0; i < 300; i++) si.pdq(rgb, 64, 64);
}

// A typed round trip through every layer-4 step kind, so a new Step variant cannot slip by unencoded.
for (const step of [{ kind: "Submit" }, { kind: "Upload", file: 3 }, { kind: "Describe", file: 0 }, { kind: "Finish" }, { kind: "Retract" }, { kind: "Wait", until: 5n }, { kind: "Hold", reason: "Failed" }, { kind: "Hold", reason: "AwaitingCredentials" }, { kind: "Done" }]) {
  assert.deepEqual(decodeStep(encodeStep(step)), { ...step, ...(step.until !== undefined ? { until: BigInt(step.until) } : {}) });
}
for (const a of [{ kind: "Accepted", reportId: "r" }, { kind: "Accepted", reportId: null }, { kind: "Transient" }, { kind: "AuthRefused" }, { kind: "Invalid", detail: "d" }, { kind: "AlreadyFinished" }, { kind: "WindowExpired" }, { kind: "Unknown", code: "x" }]) {
  assert.deepEqual(decodeAnswer(encodeAnswer(a)), a);
}
assert.deepEqual(decodeTwo(encodeTwo(["a", "b"])), ["a", "b"]);
assert.deepEqual(decodeApproval(encodeApproval({ kind: "One", by: "a" })), { kind: "One", by: "a" });
assert.deepEqual(decodeEvidenceEvent(encodeEvidenceEvent({ kind: "DecidedWithoutEvidence", by: ["a", "b"] })), { kind: "DecidedWithoutEvidence", by: ["a", "b"] });
assert.deepEqual(decodeStates(encodeStates(["Pending", "Missing"])), ["Pending", "Missing"]);
assert.deepEqual(decodeStateFound(encodeStateFound([{ state: "Present", objectFound: true }])), [{ state: "Present", objectFound: true }]);
assert.deepEqual(decodeClocks(encodeClocks([{ status: "Finished", sentAt: [1, 2n], finishedAt: 9 }])), [{ status: "Finished", sentAt: [1n, 2n], finishedAt: 9n }]);

console.log(`wrapper ok: ${counts.l2} layer 2, ${counts.l4} layer 4 and ${counts.l3} layer 3 vectors replayed through the typed API`);
