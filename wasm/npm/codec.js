// SPDX-License-Identifier: MIT OR Apache-2.0
// Binary encoding of the structured types that cross the boundary (docs/WASM-BOUNDARY.md section 3).
// Mirrors wasm/src/codec.rs, report_codec.rs and evidence_codec.rs; wasm/npm/test.mjs replays the
// repository's conformance vectors through it so the two cannot drift.
//
// Conventions: version byte 1 first; integers little-endian; a string is a u32 byte length then
// UTF-8; an option is a 0/1 byte then the value; an enum is a tag byte then its payload.
// Times are u64 seconds and are `bigint` on the way out; `number | bigint` is accepted on the way in.
//
// Fail closed: where the Rust codec refuses an unknown tag, so does this one (it throws). Where the
// Rust codec maps an unknown tag to the safe value (an unknown outcome is Unavailable, an unknown
// answer is Unknown), so does this one. Nothing here ever turns an unknown tag into "clean".

/** Numeric refusal codes of the module, by name. A negative return is always a refusal: hold. */
export const CODES = Object.freeze({
  ZERO_DIMENSION: -1,
  TOO_LARGE: -2,
  LENGTH_MISMATCH: -3,
  NULL_POINTER: -4,
  INVALID_FRAME: -5,
  INPUT_TOO_LARGE: -6,
  OUT_TOO_SMALL: -7,
  BAD_VERSION: -8,
  TRUNCATED: -9,
  UNKNOWN_TAG: -10,
  MALFORMED: -11,
  TRAILING_BYTES: -12,
  BAD_ARGUMENT: -13,
  ALREADY_FINISHED: -14,
  REFUSED: -15,
  SAME_PERSON: -16,
  CASE_DESTROYED: -20,
  CASE_ALREADY_DECIDED: -21,
  CASE_NOT_CLEARABLE: -22,
  CASE_NEEDS_TWO_PEOPLE: -23,
  CASE_NO_SUCH_FILE: -24,
  TRANSITION_REFUSED: -25,
  CASE_HOLD_ALREADY_SET: -26,
  CASE_NO_HOLD: -27,
  CASE_BLANK: -28,
  CASE_DESTRUCTION_ALREADY_BEGUN: -29,
  INELIGIBLE_UNDECIDED: -31,
  INELIGIBLE_CONFIRMED: -32,
  INELIGIBLE_BEFORE_END: -33,
  INELIGIBLE_LEGAL_HOLD: -34,
  INELIGIBLE_SHARED_FILE: -35,
  INELIGIBLE_ALREADY_BEGUN: -36,
  RELEASE_NOT_CLEARED: -40,
  RELEASE_EVIDENCE_COPY_NOT_HELD: -41,
  RELEASE_BEFORE_END: -42,
  RELEASE_LEGAL_HOLD: -43,
});

const NAME_OF = new Map(Object.entries(CODES).map(([name, code]) => [code, name]));

/** A refusal from the module or from the encoding. Treat as "hold": nothing is resolved toward clean. */
export class SafeImageError extends Error {
  /** @param {number} code a negative value from {@link CODES} (or an unrecognised one from a newer module) */
  constructor(code, where) {
    const name = NAME_OF.get(code) ?? "UNRECOGNISED";
    super(`SafeImage refused${where ? ` (${where})` : ""}: ${name} (${code})`);
    this.name = "SafeImageError";
    this.code = code;
    this.codeName = name;
  }
}

const refuse = (name) => new SafeImageError(CODES[name], "encoding");

const U32_MAX = 0xffffffff;
const U64_MAX = (1n << 64n) - 1n;
const textEncoder = new TextEncoder();
// `fatal` so invalid UTF-8 is an error, as in Rust's `from_utf8`.
const textDecoder = new TextDecoder("utf-8", { fatal: true });

/** @param {number | bigint} v */
export function toU64(v) {
  let b;
  try {
    b = typeof v === "bigint" ? v : Number.isSafeInteger(v) ? BigInt(v) : null;
  } catch {
    b = null;
  }
  if (b === null || b < 0n || b > U64_MAX) throw new RangeError(`not a u64: ${String(v)}`);
  return b;
}

/** @param {number} v */
export function toU32(v) {
  if (!Number.isInteger(v) || v < 0 || v > U32_MAX) throw new RangeError(`not a u32: ${String(v)}`);
  return v;
}

export class Writer {
  constructor() {
    this.parts = [];
    this.parts.push(Uint8Array.of(1)); // version
  }
  u8(v) {
    this.parts.push(Uint8Array.of(v));
  }
  u32(v) {
    const b = new Uint8Array(4);
    new DataView(b.buffer).setUint32(0, toU32(v), true);
    this.parts.push(b);
  }
  u64(v) {
    const b = new Uint8Array(8);
    new DataView(b.buffer).setBigUint64(0, toU64(v), true);
    this.parts.push(b);
  }
  str(s) {
    const b = textEncoder.encode(s);
    this.u32(b.length);
    this.parts.push(b);
  }
  raw(b) {
    this.parts.push(b);
  }
  flag(v) {
    this.u8(v ? 1 : 0);
  }
  optStr(s) {
    if (s === null || s === undefined) this.u8(0);
    else {
      this.u8(1);
      this.str(s);
    }
  }
  optU64(v) {
    if (v === null || v === undefined) this.u8(0);
    else {
      this.u8(1);
      this.u64(v);
    }
  }
  finish() {
    let n = 0;
    for (const p of this.parts) n += p.length;
    const out = new Uint8Array(n);
    let at = 0;
    for (const p of this.parts) {
      out.set(p, at);
      at += p.length;
    }
    return out;
  }
}

export class Reader {
  /** Checks the version byte. @param {Uint8Array} bytes */
  constructor(bytes) {
    this.b = bytes;
    this.at = 0;
    if (bytes.length === 0) throw refuse("TRUNCATED");
    if (bytes[0] !== 1) throw refuse("BAD_VERSION");
    this.at = 1;
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }
  take(n) {
    if (this.b.length - this.at < n) throw refuse("TRUNCATED");
    const s = this.b.subarray(this.at, this.at + n);
    this.at += n;
    return s;
  }
  end() {
    if (this.at !== this.b.length) throw refuse("TRAILING_BYTES");
  }
  u8() {
    return this.take(1)[0];
  }
  u32() {
    const at = this.at;
    this.take(4);
    return this.view.getUint32(at, true);
  }
  u64() {
    const at = this.at;
    this.take(8);
    return this.view.getBigUint64(at, true);
  }
  str() {
    const b = this.take(this.u32());
    try {
      return textDecoder.decode(b);
    } catch {
      throw refuse("MALFORMED");
    }
  }
  flag() {
    const v = this.u8();
    if (v > 1) throw refuse("MALFORMED");
    return v === 1;
  }
  optStr() {
    return this.flag() ? this.str() : null;
  }
  optU64() {
    return this.flag() ? this.u64() : null;
  }
  raw(n) {
    return this.take(n);
  }
}

// ---- layer 2 ----------------------------------------------------------------------------------

/** Unavailable reasons, indexed by wire tag (also the value `si_classify_status` returns minus one). */
export const UNAVAILABLE = Object.freeze([
  "NotConfigured",
  "Disabled",
  "Timeout",
  "ServerError",
  "Throttled",
  "CredentialsRefused",
  "MalformedResponse",
  "TestValueInProduction",
]);

/** An unknown tag is MalformedResponse: still unavailable, never clean. */
export const unavailableFromTag = (t) => UNAVAILABLE[t] ?? "MalformedResponse";

export function unavailableTag(reason) {
  const t = UNAVAILABLE.indexOf(reason);
  if (t < 0) throw new TypeError(`unknown Unavailable reason: ${String(reason)}`);
  return t;
}

function putOutcome(w, o) {
  switch (o.kind) {
    case "Clean":
      w.u8(0);
      w.optStr(o.listVersion);
      w.optU64(o.checkedAt);
      return;
    case "Unavailable":
      w.u8(1);
      w.u8(unavailableTag(o.reason));
      return;
    case "Match": {
      w.u8(2);
      const c = o.classification;
      if (c.kind === "Other") {
        w.u8(0);
        w.str(c.label);
      } else if (c.kind === "HarmfulAbusive") w.u8(1);
      else if (c.kind === "Csam") w.u8(2);
      else throw new TypeError(`unknown classification: ${String(c.kind)}`);
      if (o.matchType === "Near") w.u8(0);
      else if (o.matchType === "Exact") w.u8(1);
      else throw new TypeError(`unknown match type: ${String(o.matchType)}`);
      w.u32(o.details.length);
      for (const d of o.details) {
        w.u32(d.digests.length);
        for (const g of d.digests) {
          w.str(g.algorithm);
          w.str(g.value);
        }
        w.optStr(d.listedAt);
      }
      return;
    }
    default:
      throw new TypeError(`unknown outcome kind: ${String(o.kind)}`);
  }
}

function getOutcome(r) {
  switch (r.u8()) {
    case 0:
      return { kind: "Clean", listVersion: r.optStr(), checkedAt: r.optU64() };
    case 1:
      return { kind: "Unavailable", reason: unavailableFromTag(r.u8()) };
    case 2: {
      let classification;
      switch (r.u8()) {
        case 0:
          classification = { kind: "Other", label: r.str() };
          break;
        case 1:
          classification = { kind: "HarmfulAbusive" };
          break;
        case 2:
          classification = { kind: "Csam" };
          break;
        default:
          throw refuse("UNKNOWN_TAG");
      }
      const t = r.u8();
      if (t > 1) throw refuse("UNKNOWN_TAG");
      const matchType = t === 0 ? "Near" : "Exact";
      const details = [];
      for (let n = r.u32(); n > 0; n--) {
        const digests = [];
        for (let m = r.u32(); m > 0; m--) digests.push({ algorithm: r.str(), value: r.str() });
        details.push({ digests, listedAt: r.optStr() });
      }
      return { kind: "Match", classification, matchType, details };
    }
    // Fails closed: unavailable, never clean. The rest of the message is not trusted.
    default:
      return { kind: "Unavailable", reason: "MalformedResponse" };
  }
}

export function encodeOutcome(o) {
  const w = new Writer();
  putOutcome(w, o);
  return w.finish();
}

export function decodeOutcome(bytes) {
  const r = new Reader(bytes);
  const o = getOutcome(r);
  // An unknown outcome tag leaves its payload unread; the Rust decoder refuses trailing bytes too.
  r.end();
  return o;
}

export function encodeOutcomes(list) {
  const w = new Writer();
  w.u32(list.length);
  for (const o of list) putOutcome(w, o);
  return w.finish();
}

export function decodeOutcomes(bytes) {
  const r = new Reader(bytes);
  const out = [];
  for (let n = r.u32(); n > 0; n--) out.push(getOutcome(r));
  r.end();
  return out;
}

/** @param {Uint8Array[]} hashes 32-byte hashes in the layer 1 layout */
export function encodeHashes(hashes) {
  const w = new Writer();
  w.u32(hashes.length);
  for (const h of hashes) {
    if (h.length !== 32) throw new TypeError("a hash is 32 bytes");
    w.raw(h);
  }
  return w.finish();
}

// ---- layer 4 ----------------------------------------------------------------------------------

export const STATUS = Object.freeze([
  "AwaitingCredentials",
  "Pending",
  "Submitted",
  "Finished",
  "Failed",
  "RetractPending",
  "Withdrawn",
]);

const statusTag = (s) => {
  const t = STATUS.indexOf(s);
  if (t < 0) throw new TypeError(`unknown report status: ${String(s)}`);
  return t;
};
const statusFromTag = (t) => {
  if (t >= STATUS.length) throw refuse("UNKNOWN_TAG");
  return STATUS[t];
};

const FINISH_NOTES = [null, "AlreadyFinished", "FinishedBeforeRetract"];

function putState(w, s) {
  w.u8(statusTag(s.status));
  w.u32(s.attempts);
  w.optU64(s.nextAttemptAt);
  w.optStr(s.reportId);
  w.u32(s.files.length);
  for (const f of s.files) {
    w.flag(f.uploaded);
    w.flag(f.described);
  }
  w.optStr(s.lastCode);
  w.u32(s.abandonedIds.length);
  for (const id of s.abandonedIds) w.str(id);
  const n = FINISH_NOTES.indexOf(s.finishNote ?? null);
  if (n < 0) throw new TypeError(`unknown finish note: ${String(s.finishNote)}`);
  w.u8(n);
}

function getState(r) {
  const status = statusFromTag(r.u8());
  const attempts = r.u32();
  const nextAttemptAt = r.optU64();
  const reportId = r.optStr();
  const files = [];
  for (let n = r.u32(); n > 0; n--) files.push({ uploaded: r.flag(), described: r.flag() });
  const lastCode = r.optStr();
  const abandonedIds = [];
  for (let n = r.u32(); n > 0; n--) abandonedIds.push(r.str());
  const note = r.u8();
  if (note >= FINISH_NOTES.length) throw refuse("UNKNOWN_TAG");
  return { status, attempts, nextAttemptAt, reportId, files, lastCode, abandonedIds, finishNote: FINISH_NOTES[note] };
}

export function encodeState(s) {
  const w = new Writer();
  putState(w, s);
  return w.finish();
}

export function decodeState(bytes) {
  const r = new Reader(bytes);
  const s = getState(r);
  r.end();
  return s;
}

const EVENT_KINDS = ["AuthRefused", "Invalid", "WindowExpired", "UnknownAnswer", "FinishedBeforeRetract", "MissingReportId", "StepNotValidHere"];

/** The result of `apply`: the new state, then an optional event. */
export function decodeApplied(bytes) {
  const r = new Reader(bytes);
  const state = getState(r);
  let event = null;
  if (r.flag()) {
    const t = r.u8();
    if (t === 1) event = { kind: "Invalid", detail: r.str() };
    else if (t === 3) event = { kind: "UnknownAnswer", code: r.str() };
    else if (t < EVENT_KINDS.length) event = { kind: EVENT_KINDS[t] };
    // 255 (a variant a newer core added) or anything else: the host treats it as an alarm.
    else event = { kind: "Unknown" };
  }
  r.end();
  return { state, event };
}

export function encodeStep(s) {
  const w = new Writer();
  switch (s.kind) {
    case "Submit":
      w.u8(0);
      break;
    case "Upload":
      w.u8(1);
      w.u32(s.file);
      break;
    case "Describe":
      w.u8(2);
      w.u32(s.file);
      break;
    case "Finish":
      w.u8(3);
      break;
    case "Retract":
      w.u8(4);
      break;
    case "Wait":
      w.u8(5);
      w.u64(s.until);
      break;
    case "Hold":
      w.u8(6);
      if (s.reason === "AwaitingCredentials") w.u8(0);
      else if (s.reason === "Failed") w.u8(1);
      else throw new TypeError(`unknown hold reason: ${String(s.reason)}`);
      break;
    case "Done":
      w.u8(7);
      break;
    default:
      throw new TypeError(`unknown step kind: ${String(s.kind)}`);
  }
  return w.finish();
}

export function decodeStep(bytes) {
  const r = new Reader(bytes);
  let s;
  switch (r.u8()) {
    case 0:
      s = { kind: "Submit" };
      break;
    case 1:
      s = { kind: "Upload", file: r.u32() };
      break;
    case 2:
      s = { kind: "Describe", file: r.u32() };
      break;
    case 3:
      s = { kind: "Finish" };
      break;
    case 4:
      s = { kind: "Retract" };
      break;
    case 5:
      s = { kind: "Wait", until: r.u64() };
      break;
    case 6: {
      const h = r.u8();
      if (h > 1) throw refuse("UNKNOWN_TAG");
      s = { kind: "Hold", reason: h === 0 ? "AwaitingCredentials" : "Failed" };
      break;
    }
    case 7:
      s = { kind: "Done" };
      break;
    default:
      throw refuse("UNKNOWN_TAG");
  }
  r.end();
  return s;
}

export function encodeAnswer(a) {
  const w = new Writer();
  switch (a.kind) {
    case "Accepted":
      w.u8(0);
      w.optStr(a.reportId);
      break;
    case "Transient":
      w.u8(1);
      break;
    case "AuthRefused":
      w.u8(2);
      break;
    case "Invalid":
      w.u8(3);
      w.str(a.detail);
      break;
    case "AlreadyFinished":
      w.u8(4);
      break;
    case "WindowExpired":
      w.u8(5);
      break;
    case "Unknown":
      w.u8(6);
      w.str(a.code);
      break;
    default:
      throw new TypeError(`unknown answer kind: ${String(a.kind)}`);
  }
  return w.finish();
}

/** The module decodes answers itself; this is for hosts that store them. Unknown tag: Unknown. */
export function decodeAnswer(bytes) {
  const r = new Reader(bytes);
  let a;
  switch (r.u8()) {
    case 0:
      a = { kind: "Accepted", reportId: r.optStr() };
      break;
    case 1:
      a = { kind: "Transient" };
      break;
    case 2:
      a = { kind: "AuthRefused" };
      break;
    case 3:
      a = { kind: "Invalid", detail: r.str() };
      break;
    case 4:
      a = { kind: "AlreadyFinished" };
      break;
    case 5:
      a = { kind: "WindowExpired" };
      break;
    case 6:
      a = { kind: "Unknown", code: r.str() };
      break;
    default:
      return { kind: "Unknown", code: "unknown_tag" };
  }
  r.end();
  return a;
}

// ---- layer 3 ----------------------------------------------------------------------------------

export const EVIDENCE_STATE = Object.freeze(["Pending", "Present", "Failed", "Missing", "FileWithoutEvidence"]);

export function evidenceStateTag(s) {
  const t = EVIDENCE_STATE.indexOf(s);
  if (t < 0) throw new TypeError(`unknown evidence state: ${String(s)}`);
  return t;
}
const evidenceStateFromTag = (t) => {
  if (t >= EVIDENCE_STATE.length) throw refuse("UNKNOWN_TAG");
  return EVIDENCE_STATE[t];
};

const putActor = (w, a) => w.str(a);
const getActor = (r) => {
  // ActorId::new refuses blank and over-128-byte identities: Malformed.
  const s = r.str();
  if (s.trim() === "" || textEncoder.encode(s).length > 128) throw refuse("MALFORMED");
  return s;
};

function putTwo(w, t) {
  putActor(w, t[0]);
  putActor(w, t[1]);
}
function getTwo(r) {
  const a = getActor(r);
  const b = getActor(r);
  if (a === b) throw refuse("SAME_PERSON");
  return [a, b];
}

/** @param {[string, string]} pair two different actor ids */
export function encodeTwo(pair) {
  const w = new Writer();
  putTwo(w, pair);
  return w.finish();
}

export function encodeEvidenceEvent(e) {
  const w = new Writer();
  switch (e.kind) {
    case "PutSucceeded":
      w.u8(0);
      break;
    case "PutFailed":
      w.u8(1);
      break;
    case "ObjectMissing":
      w.u8(2);
      break;
    case "DecidedWithoutEvidence":
      w.u8(3);
      putTwo(w, e.by);
      break;
    default:
      throw new TypeError(`unknown evidence event: ${String(e.kind)}`);
  }
  return w.finish();
}

/** @param {{kind:"One", by:string} | {kind:"Two", by:[string,string]}} a */
export function encodeApproval(a) {
  const w = new Writer();
  if (a.kind === "One") {
    w.u8(0);
    putActor(w, a.by);
  } else if (a.kind === "Two") {
    w.u8(1);
    putTwo(w, a.by);
  } else throw new TypeError(`unknown approval: ${String(a.kind)}`);
  return w.finish();
}

export function encodeStates(states) {
  const w = new Writer();
  w.u32(states.length);
  for (const s of states) w.u8(evidenceStateTag(s));
  return w.finish();
}

export function encodeStateFound(list) {
  const w = new Writer();
  w.u32(list.length);
  for (const f of list) {
    w.u8(evidenceStateTag(f.state));
    w.flag(f.objectFound);
  }
  return w.finish();
}

export function encodeClocks(list) {
  const w = new Writer();
  w.u32(list.length);
  for (const c of list) {
    w.u8(statusTag(c.status));
    w.u32(c.sentAt.length);
    for (const t of c.sentAt) w.u64(t);
    w.optU64(c.finishedAt);
  }
  return w.finish();
}

const CASE_KINDS = ["KnownHash", "Classifier", "RemovalRequest", "ModeratorSighting"];
export const caseKindCode = (k) => {
  const t = CASE_KINDS.indexOf(k);
  if (t < 0) throw new TypeError(`unknown case kind: ${String(k)}`);
  return t;
};

export function encodeCase(c) {
  const w = new Writer();
  w.u8(caseKindCode(c.kind));
  w.u64(c.createdAt);
  if (c.review === null || c.review === undefined) w.u8(0);
  else {
    w.u8(1);
    if (c.review.outcome === "Confirmed") w.u8(0);
    else if (c.review.outcome === "FalsePositive") w.u8(1);
    else throw new TypeError(`unknown review outcome: ${String(c.review.outcome)}`);
    w.u64(c.review.at);
  }
  w.u32(c.files.length);
  for (const f of c.files) {
    w.str(f.sha256Hex);
    w.u8(evidenceStateTag(f.state));
  }
  if (c.evidenceHold === null || c.evidenceHold === undefined) w.u8(0);
  else {
    w.u8(1);
    putTwo(w, c.evidenceHold.setBy);
    w.str(c.evidenceHold.reason);
    w.u64(c.evidenceHold.setAt);
  }
  if (c.destruction === null || c.destruction === undefined) w.u8(0);
  else {
    w.u8(1);
    const b = c.destruction.basis;
    if (b.kind === "LawEnforcementRequest") {
      w.u8(0);
      w.str(b.requestRef);
    } else if (b.kind === "ClearedAndExpired") w.u8(1);
    else throw new TypeError(`unknown destruction basis: ${String(b.kind)}`);
    w.u64(c.destruction.at);
    if (c.destruction.by === null || c.destruction.by === undefined) w.u8(0);
    else {
      w.u8(1);
      putTwo(w, c.destruction.by);
    }
  }
  return w.finish();
}

export function decodeCase(bytes) {
  const r = new Reader(bytes);
  const k = r.u8();
  if (k >= CASE_KINDS.length) throw refuse("UNKNOWN_TAG");
  const kind = CASE_KINDS[k];
  const createdAt = r.u64();
  let review = null;
  if (r.flag()) {
    const o = r.u8();
    if (o > 1) throw refuse("UNKNOWN_TAG");
    review = { outcome: o === 0 ? "Confirmed" : "FalsePositive", at: r.u64() };
  }
  const files = [];
  for (let n = r.u32(); n > 0; n--) {
    const sha256Hex = r.str();
    files.push({ sha256Hex, state: evidenceStateFromTag(r.u8()) });
  }
  let evidenceHold = null;
  if (r.flag()) {
    const setBy = getTwo(r);
    const reason = r.str();
    evidenceHold = { setBy, reason, setAt: r.u64() };
  }
  let destruction = null;
  if (r.flag()) {
    let basis;
    switch (r.u8()) {
      case 0:
        basis = { kind: "LawEnforcementRequest", requestRef: r.str() };
        break;
      case 1:
        basis = { kind: "ClearedAndExpired" };
        break;
      default:
        throw refuse("UNKNOWN_TAG");
    }
    const at = r.u64();
    const by = r.flag() ? getTwo(r) : null;
    destruction = { basis, at, by };
  }
  r.end();
  return { kind, createdAt, review, files, evidenceHold, destruction };
}

// ---- decoders for the layer 3 inputs (a host that stores them reads them back with these) ------

export function decodeTwo(bytes) {
  const r = new Reader(bytes);
  const t = getTwo(r);
  r.end();
  return t;
}

export function decodeEvidenceEvent(bytes) {
  const r = new Reader(bytes);
  let e;
  switch (r.u8()) {
    case 0:
      e = { kind: "PutSucceeded" };
      break;
    case 1:
      e = { kind: "PutFailed" };
      break;
    case 2:
      e = { kind: "ObjectMissing" };
      break;
    case 3:
      e = { kind: "DecidedWithoutEvidence", by: getTwo(r) };
      break;
    default:
      throw refuse("UNKNOWN_TAG");
  }
  r.end();
  return e;
}

export function decodeApproval(bytes) {
  const r = new Reader(bytes);
  let a;
  switch (r.u8()) {
    case 0:
      a = { kind: "One", by: getActor(r) };
      break;
    case 1:
      a = { kind: "Two", by: getTwo(r) };
      break;
    default:
      throw refuse("UNKNOWN_TAG");
  }
  r.end();
  return a;
}

export function decodeStates(bytes) {
  const r = new Reader(bytes);
  const out = [];
  for (let n = r.u32(); n > 0; n--) out.push(evidenceStateFromTag(r.u8()));
  r.end();
  return out;
}

export function decodeStateFound(bytes) {
  const r = new Reader(bytes);
  const out = [];
  for (let n = r.u32(); n > 0; n--) {
    const state = evidenceStateFromTag(r.u8());
    out.push({ state, objectFound: r.flag() });
  }
  r.end();
  return out;
}

export function decodeClocks(bytes) {
  const r = new Reader(bytes);
  const out = [];
  for (let n = r.u32(); n > 0; n--) {
    const status = statusFromTag(r.u8());
    const sentAt = [];
    for (let m = r.u32(); m > 0; m--) sentAt.push(r.u64());
    out.push({ status, sentAt, finishedAt: r.optU64() });
  }
  r.end();
  return out;
}
