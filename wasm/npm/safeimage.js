// Typed, synchronous wrapper over the `si_*` exports (docs/WASM-BOUNDARY.md). It owns the calling
// convention (alloc, copy in, call, copy out, free) and the encodings, so a host never touches a
// pointer. Every refusal of the module is a thrown `SafeImageError`; treat it as "hold".
//
// Times are u64 seconds: pass `number | bigint`, read `bigint`.

import { instantiate } from "./loader.js";
import {
  CODES,
  SafeImageError,
  decodeApplied,
  decodeCase,
  decodeOutcome,
  decodeState,
  decodeStep,
  encodeApproval,
  encodeCase,
  encodeClocks,
  encodeAnswer,
  encodeEvidenceEvent,
  encodeHashes,
  encodeOutcomes,
  encodeState,
  encodeStateFound,
  encodeStates,
  encodeStep,
  encodeTwo,
  evidenceStateTag,
  toU32,
  toU64,
  unavailableFromTag,
  unavailableTag,
} from "./codec.js";

const HEADROOM = 1024; // si_report_apply and the si_case_* writers may grow their input by this much
const textEncoder = new TextEncoder();

const INELIGIBLE = new Map([
  [CODES.INELIGIBLE_UNDECIDED, "Undecided"],
  [CODES.INELIGIBLE_CONFIRMED, "Confirmed"],
  [CODES.INELIGIBLE_BEFORE_END, "BeforeEnd"],
  [CODES.INELIGIBLE_LEGAL_HOLD, "LegalHold"],
  [CODES.INELIGIBLE_SHARED_FILE, "SharedFile"],
  [CODES.INELIGIBLE_ALREADY_BEGUN, "AlreadyBegun"],
]);
const RELEASE = new Map([
  [CODES.RELEASE_NOT_CLEARED, "NotCleared"],
  [CODES.RELEASE_EVIDENCE_COPY_NOT_HELD, "EvidenceCopyNotHeld"],
  [CODES.RELEASE_BEFORE_END, "BeforeEnd"],
  [CODES.RELEASE_LEGAL_HOLD, "LegalHold"],
]);

/** Upstream text form of a hash: 64 hex digits, word 15 first. */
export function hashToHex(hash) {
  let s = "";
  for (let i = 15; i >= 0; i--) {
    const w = hash[i * 2] | (hash[i * 2 + 1] << 8);
    s += w.toString(16).padStart(4, "0");
  }
  return s;
}

/** The inverse of {@link hashToHex}. */
export function hashFromHex(text) {
  if (!/^[0-9a-fA-F]{64}$/.test(text)) throw new TypeError("a hash is 64 hex digits");
  const out = new Uint8Array(32);
  for (let i = 0; i < 16; i++) {
    const w = parseInt(text.slice((15 - i) * 4, (15 - i) * 4 + 4), 16);
    out[i * 2] = w & 0xff;
    out[i * 2 + 1] = w >> 8;
  }
  return out;
}

export class SafeImage {
  #x;

  /** Use {@link SafeImage.create}. @param {object} exports */
  constructor(exports) {
    this.#x = exports;
  }

  /**
   * @param {BufferSource | WebAssembly.Module} source the module's bytes, or an already compiled
   *   module (a worker host compiles it at deploy time and passes it in).
   */
  static async create(source) {
    return new SafeImage(await instantiate(source));
  }

  /**
   * Runs one export. `inputs` are copied into module memory, `outCap` bytes are reserved for the
   * output, and `fn` is called with the input pointers then the output pointer. A negative return
   * throws. A non-negative return is the byte count (or the value, for exports with no output).
   * Views over module memory are taken only after every allocation: an allocation can grow, and so
   * detach, the memory buffer.
   */
  #run(where, inputs, outCap, fn) {
    const x = this.#x;
    const ptrs = [];
    let outPtr = 0;
    try {
      for (const b of inputs) ptrs.push(x.si_alloc(b.length));
      if (outCap > 0) outPtr = x.si_alloc(outCap);
      inputs.forEach((b, i) => new Uint8Array(x.memory.buffer, ptrs[i], b.length).set(b));
      const rc = fn(ptrs, outPtr);
      if (rc < 0) throw new SafeImageError(rc, where);
      const out = outCap > 0 && rc > 0 ? new Uint8Array(x.memory.buffer, outPtr, Math.min(rc, outCap)).slice() : new Uint8Array(0);
      return { rc, out };
    } finally {
      inputs.forEach((b, i) => {
        if (i < ptrs.length) x.si_free(ptrs[i], b.length);
      });
      if (outPtr !== 0) x.si_free(outPtr, outCap);
    }
  }

  /** The module's ABI version. */
  get abiVersion() {
    return this.#x.si_abi_version();
  }

  // ---- layer 1 ------------------------------------------------------------------------------

  /**
   * PDQ hash of one RGB frame (3 bytes per pixel, row-major).
   * @returns {{ hash: Uint8Array, quality: number }}
   */
  pdq(rgb, width, height) {
    const { out } = this.#run("pdq", [rgb], 33, ([p], o) => {
      const rc = this.#x.si_pdq(p, rgb.length, toU32(width), toU32(height), o);
      return rc === 0 ? 33 : rc;
    });
    return { hash: out.slice(0, 32), quality: out[32] };
  }

  /**
   * PDQ hashes of one frame in all eight orientations.
   * @returns {{ hashes: Uint8Array[], quality: number }}
   */
  pdqDihedral(rgb, width, height) {
    const { out } = this.#run("pdqDihedral", [rgb], 257, ([p], o) => {
      const rc = this.#x.si_pdq_dihedral(p, rgb.length, toU32(width), toU32(height), o);
      return rc === 0 ? 257 : rc;
    });
    return { hashes: Array.from({ length: 8 }, (_, i) => out.slice(i * 32, i * 32 + 32)), quality: out[256] };
  }

  // ---- layer 2 ------------------------------------------------------------------------------

  /** The strongest of several outcomes; an empty list is Unavailable(NotConfigured). */
  strongest(outcomes) {
    const input = encodeOutcomes(outcomes);
    const cap = input.length + 16;
    const { out } = this.#run("strongest", [input], cap, ([p], o) => this.#x.si_strongest(p, input.length, o, cap));
    return decodeOutcome(out);
  }

  /** `null` for a 2xx status, otherwise the reason the provider is unavailable. */
  classifyStatus(status) {
    const rc = this.#x.si_classify_status(toU32(status));
    return rc === 0 ? null : unavailableFromTag(rc - 1);
  }

  /** Whether a body of `bodyLen` bytes is within `cap`. */
  withinCap(bodyLen, cap) {
    return this.#x.si_within_cap(toU32(bodyLen), toU32(cap)) === 1;
  }

  /**
   * @param {{ maxRetries: number, delayMs: number | bigint }} policy
   * @returns {{ retry: false } | { retry: true, afterMs: bigint }}
   */
  retryDecide(policy, attemptsMade, reason, elapsedMs, deadlineMs) {
    const { out } = this.#run("retryDecide", [], 9, (_p, o) => {
      return this.#x.si_retry_decide(
        toU32(policy.maxRetries),
        toU64(policy.delayMs),
        toU32(attemptsMade),
        unavailableTag(reason),
        toU64(elapsedMs),
        toU64(deadlineMs),
        o,
      );
    });
    return out[0] === 1 ? { retry: true, afterMs: new DataView(out.buffer).getBigUint64(1, true) } : { retry: false };
  }

  /** Validates a request of 1 to 8 hashes (32 bytes each). Returns the count; throws otherwise. */
  validateMatchRequest(hashes) {
    const input = encodeHashes(hashes);
    return this.#run("validateMatchRequest", [input], 0, ([p]) => this.#x.si_match_request(p, input.length)).rc;
  }

  // ---- layer 4 ------------------------------------------------------------------------------

  /** A new report state with `fileCount` files. */
  reportNew(fileCount, credentialsReady) {
    const { out } = this.#run("reportNew", [], HEADROOM, (_p, o) =>
      this.#x.si_report_new(toU32(fileCount), credentialsReady ? 1 : 0, o, HEADROOM));
    return decodeState(out);
  }

  /** What is due next at time `now`. @returns a Step */
  reportNextStep(state, now) {
    const s = encodeState(state);
    const { out } = this.#run("reportNextStep", [s], 64, ([p], o) => this.#x.si_report_next_step(p, s.length, toU64(now), o, 64));
    return decodeStep(out);
  }

  /** Applies the destination's answer to a step. @returns {{ state, event }} (`event` may be null) */
  reportApply(state, step, answer, now) {
    const [s, t, a] = [encodeState(state), encodeStep(step), encodeAnswer(answer)];
    const cap = s.length + HEADROOM;
    const { out } = this.#run("reportApply", [s, t, a], cap, ([ps, pt, pa], o) =>
      this.#x.si_report_apply(ps, s.length, pt, t.length, pa, a.length, toU64(now), o, cap));
    return decodeApplied(out);
  }

  /** Moves a report between awaiting-credentials and pending. */
  reportSyncCredentials(state, ready) {
    const s = encodeState(state);
    const cap = s.length + HEADROOM;
    const { out } = this.#run("reportSyncCredentials", [s], cap, ([p], o) =>
      this.#x.si_report_sync_credentials(p, s.length, ready ? 1 : 0, o, cap));
    return decodeState(out);
  }

  /** Requests a withdrawal. Throws `ALREADY_FINISHED` when the report already finished. */
  reportRequestRetract(state) {
    const s = encodeState(state);
    const cap = s.length + HEADROOM;
    const { out } = this.#run("reportRequestRetract", [s], cap, ([p], o) => this.#x.si_report_request_retract(p, s.length, o, cap));
    return decodeState(out);
  }

  /** A human's retry of a failed report. */
  reportRetryFailed(state) {
    const s = encodeState(state);
    const cap = s.length + HEADROOM;
    const { out } = this.#run("reportRetryFailed", [s], cap, ([p], o) => this.#x.si_report_retry_failed(p, s.length, o, cap));
    return decodeState(out);
  }

  /** Seconds to wait after `attempts` consecutive failures. @returns bigint */
  backoffSecs(attempts) {
    return this.#x.si_backoff_secs(toU32(attempts));
  }

  /** Whether an unfinished report may have been discarded by the destination. */
  deletionWindowElapsed(openedAt, lastModifiedAt, now, policy) {
    return (
      this.#x.si_deletion_window_elapsed(toU64(openedAt), toU64(lastModifiedAt), toU64(now), toU64(policy.openSecs), toU64(policy.idleSecs)) === 1
    );
  }

  // ---- layer 3 ------------------------------------------------------------------------------

  /** The evidence state after an event; throws `TRANSITION_REFUSED` for an event not valid here. */
  evidenceTransition(state, event) {
    const e = encodeEvidenceEvent(event);
    const rc = this.#run("evidenceTransition", [e], 0, ([p]) => this.#x.si_evidence_transition(evidenceStateTag(state), p, e.length)).rc;
    return ["Pending", "Present", "Failed", "Missing", "FileWithoutEvidence"][rc];
  }

  /** A case's evidence state derived from its files' states: "Failed" | "Pending" | "Present". */
  caseEvidence(states) {
    const s = encodeStates(states);
    const rc = this.#run("caseEvidence", [s], 0, ([p]) => this.#x.si_case_evidence(p, s.length)).rc;
    return ["Failed", "Pending", "Present"][rc];
  }

  /** What the drain may do with one report file: "SendWithFile" | "SendWithoutFile" | "Hold". */
  reportFileDisposition(state, objectFound) {
    const rc = this.#x.si_report_file_disposition(evidenceStateTag(state), objectFound ? 1 : 0);
    if (rc < 0) throw new SafeImageError(rc, "reportFileDisposition");
    return ["SendWithFile", "SendWithoutFile", "Hold"][rc];
  }

  /** Whether a report may be submitted: it has files and none holds it. @param {{state, objectFound}[]} files */
  reportMaySubmit(files) {
    const f = encodeStateFound(files);
    return this.#run("reportMaySubmit", [f], 0, ([p]) => this.#x.si_report_may_submit(p, f.length)).rc === 1;
  }

  /** One preservation period, in seconds. @returns bigint */
  preservationPeriodSecs() {
    return this.#x.si_preservation_period_secs();
  }

  /**
   * The earliest time evidence may be released, or `null` meaning **keep everything**.
   * @param {{status, sentAt: (number|bigint)[], finishedAt: number|bigint|null}[]} clocks
   */
  preserveUntil(matchAt, decided, clocks) {
    const c = encodeClocks(clocks);
    const { out } = this.#run("preserveUntil", [c], 9, ([p], o) =>
      this.#x.si_preserve_until(toU64(matchAt), decided ? 1 : 0, p, c.length, o));
    return out[0] === 1 ? new DataView(out.buffer).getBigUint64(1, true) : null;
  }

  #caseOp(where, inputs, extra, fn) {
    const total = inputs.reduce((n, b) => n + b.length, 0);
    const cap = total + extra + HEADROOM;
    const { out } = this.#run(where, inputs, cap, (ps, o) => fn(ps, o, cap));
    return decodeCase(out);
  }

  /** Applies an evidence event to file `index` of a case. Returns the new case. */
  caseApplyFileEvent(theCase, index, event) {
    const [c, e] = [encodeCase(theCase), encodeEvidenceEvent(event)];
    return this.#caseOp("caseApplyFileEvent", [c, e], 0, ([pc, pe], o, cap) =>
      this.#x.si_case_apply_file_event(pc, c.length, toU32(index), pe, e.length, o, cap));
  }

  /** Records a confirmation. */
  caseConfirm(theCase, at) {
    const c = encodeCase(theCase);
    return this.#caseOp("caseConfirm", [c], 0, ([pc], o, cap) => this.#x.si_case_confirm(pc, c.length, toU64(at), o, cap));
  }

  /** Records a clear (false positive). @param approval `{kind:"One", by}` or `{kind:"Two", by:[a,b]}` */
  caseClear(theCase, approval, at) {
    const [c, a] = [encodeCase(theCase), encodeApproval(approval)];
    return this.#caseOp("caseClear", [c, a], 0, ([pc, pa], o, cap) =>
      this.#x.si_case_clear(pc, c.length, pa, a.length, toU64(at), o, cap));
  }

  /** Sets an evidence legal hold. `by` is two different actor ids. */
  caseSetHold(theCase, by, reason, at) {
    const [c, b, r] = [encodeCase(theCase), encodeTwo(by), textEncoder.encode(reason)];
    return this.#caseOp("caseSetHold", [c, b, r], 0, ([pc, pb, pr], o, cap) =>
      this.#x.si_case_set_hold(pc, c.length, pb, b.length, pr, r.length, toU64(at), o, cap));
  }

  /** Lifts an evidence legal hold. Two different people. */
  caseLiftHold(theCase, by) {
    const [c, b] = [encodeCase(theCase), encodeTwo(by)];
    return this.#caseOp("caseLiftHold", [c, b], 0, ([pc, pb], o, cap) => this.#x.si_case_lift_hold(pc, c.length, pb, b.length, o, cap));
  }

  /** Records the start of destruction on a law-enforcement request. **Records only; deletes nothing.** */
  caseBeginDestructionOnRequest(theCase, requestRef, by, at) {
    const [c, r, b] = [encodeCase(theCase), textEncoder.encode(requestRef), encodeTwo(by)];
    return this.#caseOp("caseBeginDestructionOnRequest", [c, r, b], 0, ([pc, pr, pb], o, cap) =>
      this.#x.si_case_begin_destruction_on_request(pc, c.length, pr, r.length, pb, b.length, toU64(at), o, cap));
  }

  /** Records the start of destruction of cleared evidence past its date. **Records only.** */
  caseBeginDestructionExpired(theCase, until, now, sharesDigest) {
    const c = encodeCase(theCase);
    return this.#caseOp("caseBeginDestructionExpired", [c], 0, ([pc], o, cap) =>
      this.#x.si_case_begin_destruction_expired(pc, c.length, until === null ? 0 : 1, toU64(until ?? 0), toU64(now), sharesDigest ? 1 : 0, o, cap));
  }

  /**
   * `null` when the reaper may begin destroying this case's evidence, else the reason it may not:
   * "Undecided" | "Confirmed" | "BeforeEnd" | "LegalHold" | "SharedFile" | "AlreadyBegun".
   * Any other refusal throws.
   */
  caseReaperEligibility(theCase, until, now, sharesDigest) {
    const c = encodeCase(theCase);
    const rc = this.#code("caseReaperEligibility", [c], ([pc]) =>
      this.#x.si_case_reaper_eligibility(pc, c.length, until === null ? 0 : 1, toU64(until ?? 0), toU64(now), sharesDigest ? 1 : 0));
    if (rc === 0) return null;
    const why = INELIGIBLE.get(rc);
    if (why === undefined) throw new SafeImageError(rc, "caseReaperEligibility");
    return why;
  }

  /**
   * `null` when a media hold may be released, else "NotCleared" | "EvidenceCopyNotHeld" |
   * "BeforeEnd" | "LegalHold". `target` is "ServingKey" or "EvidenceKey".
   */
  checkReleaseHold(theCase, target, until, now, evidenceCopiesHeld, by) {
    const [c, b] = [encodeCase(theCase), encodeTwo(by)];
    const t = target === "ServingKey" ? 0 : target === "EvidenceKey" ? 1 : -1;
    if (t < 0) throw new TypeError(`unknown hold target: ${String(target)}`);
    const rc = this.#code("checkReleaseHold", [c, b], ([pc, pb]) =>
      this.#x.si_check_release_hold(pc, c.length, t, until === null ? 0 : 1, toU64(until ?? 0), toU64(now), evidenceCopiesHeld ? 1 : 0, pb, b.length));
    if (rc === 0) return null;
    const why = RELEASE.get(rc);
    if (why === undefined) throw new SafeImageError(rc, "checkReleaseHold");
    return why;
  }

  /** Like #run for exports whose negative return is a result the caller interprets. */
  #code(_where, inputs, fn) {
    const x = this.#x;
    const ptrs = [];
    try {
      for (const b of inputs) ptrs.push(x.si_alloc(b.length));
      inputs.forEach((b, i) => new Uint8Array(x.memory.buffer, ptrs[i], b.length).set(b));
      return fn(ptrs);
    } finally {
      inputs.forEach((b, i) => {
        if (i < ptrs.length) x.si_free(ptrs[i], b.length);
      });
    }
  }
}
