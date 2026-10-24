// SPDX-License-Identifier: MIT OR Apache-2.0
export declare const ABI_VERSION: 4;

/** The module's raw exports: linear memory plus the `si_*` functions (docs/WASM-BOUNDARY.md). */
export interface SafeImageExports {
  memory: WebAssembly.Memory;
  si_abi_version(): number;
  si_alloc(len: number): number;
  si_free(ptr: number, len: number): void;
  [name: `si_${string}`]: (...args: number[]) => number | bigint | void;
}

/** Throws if the module's ABI version differs from `ABI_VERSION`. Most hosts want `SafeImage.create`. */
export declare function instantiate(source: BufferSource | WebAssembly.Module): Promise<SafeImageExports>;

// ---- errors -------------------------------------------------------------------------------------

/** Numeric refusal codes of the module, by name. */
export declare const CODES: Readonly<{
  ZERO_DIMENSION: -1; TOO_LARGE: -2; LENGTH_MISMATCH: -3; NULL_POINTER: -4; INVALID_FRAME: -5;
  INPUT_TOO_LARGE: -6; OUT_TOO_SMALL: -7; BAD_VERSION: -8; TRUNCATED: -9; UNKNOWN_TAG: -10;
  MALFORMED: -11; TRAILING_BYTES: -12; BAD_ARGUMENT: -13; ALREADY_FINISHED: -14; REFUSED: -15;
  SAME_PERSON: -16; CASE_DESTROYED: -20; CASE_ALREADY_DECIDED: -21; CASE_NOT_CLEARABLE: -22;
  CASE_NEEDS_TWO_PEOPLE: -23; CASE_NO_SUCH_FILE: -24; TRANSITION_REFUSED: -25;
  CASE_HOLD_ALREADY_SET: -26; CASE_NO_HOLD: -27; CASE_BLANK: -28; CASE_DESTRUCTION_ALREADY_BEGUN: -29;
  INELIGIBLE_UNDECIDED: -31; INELIGIBLE_CONFIRMED: -32; INELIGIBLE_BEFORE_END: -33;
  INELIGIBLE_LEGAL_HOLD: -34; INELIGIBLE_SHARED_FILE: -35; INELIGIBLE_ALREADY_BEGUN: -36;
  RELEASE_NOT_CLEARED: -40; RELEASE_EVIDENCE_COPY_NOT_HELD: -41; RELEASE_BEFORE_END: -42;
  RELEASE_LEGAL_HOLD: -43;
}>;

/**
 * A refusal from the module or from the encoding. It always means **hold**: nothing is resolved
 * toward clean, cleared or deleted on an error. A bad argument type or range is a `TypeError` or
 * `RangeError` instead, because that is the caller's bug and not a refusal.
 */
export declare class SafeImageError extends Error {
  readonly code: number;
  /** The name from {@link CODES}, or "UNRECOGNISED" for a code from a newer module. */
  readonly codeName: string;
  constructor(code: number, where?: string);
}

// ---- types --------------------------------------------------------------------------------------

/** Unix seconds. Read back as `bigint`; `number` (a safe integer) is accepted on the way in. */
export type Time = number | bigint;

export type Unavailable =
  | "NotConfigured" | "Disabled" | "Timeout" | "ServerError" | "Throttled"
  | "CredentialsRefused" | "MalformedResponse" | "TestValueInProduction";
export declare const UNAVAILABLE: readonly Unavailable[];

export type Classification = { kind: "Other"; label: string } | { kind: "HarmfulAbusive" } | { kind: "Csam" };
export interface Digest { algorithm: string; value: string }
export interface MatchDetail { digests: Digest[]; listedAt: string | null }
/** The result of asking a provider about one image. An unknown tag decodes to `Unavailable`, never `Clean`. */
export type MatchOutcome =
  | { kind: "Clean"; listVersion: string | null; checkedAt: Time | null }
  | { kind: "Unavailable"; reason: Unavailable }
  | { kind: "Match"; classification: Classification; matchType: "Near" | "Exact"; details: MatchDetail[] };

export type RetryDecision = { retry: false } | { retry: true; afterMs: bigint };

export type ReportStatus = "AwaitingCredentials" | "Pending" | "Submitted" | "Finished" | "Failed" | "RetractPending" | "Withdrawn";
export interface FileProgress { uploaded: boolean; described: boolean }
export interface ReportState {
  status: ReportStatus;
  attempts: number;
  nextAttemptAt: Time | null;
  reportId: string | null;
  files: FileProgress[];
  lastCode: string | null;
  abandonedIds: string[];
  finishNote: "AlreadyFinished" | "FinishedBeforeRetract" | null;
}
export type Step =
  | { kind: "Submit" } | { kind: "Upload"; file: number } | { kind: "Describe"; file: number }
  | { kind: "Finish" } | { kind: "Retract" } | { kind: "Wait"; until: Time }
  | { kind: "Hold"; reason: "AwaitingCredentials" | "Failed" } | { kind: "Done" };
export type DestinationAnswer =
  | { kind: "Accepted"; reportId: string | null } | { kind: "Transient" } | { kind: "AuthRefused" }
  | { kind: "Invalid"; detail: string } | { kind: "AlreadyFinished" } | { kind: "WindowExpired" }
  | { kind: "Unknown"; code: string };
/** What `reportApply` noticed. "Unknown" is a variant a newer core added: treat it as an alarm. */
export type ReportEvent =
  | { kind: "AuthRefused" } | { kind: "Invalid"; detail: string } | { kind: "WindowExpired" }
  | { kind: "UnknownAnswer"; code: string } | { kind: "FinishedBeforeRetract" } | { kind: "MissingReportId" }
  | { kind: "StepNotValidHere" } | { kind: "Unknown" };

export type EvidenceState = "Pending" | "Present" | "Failed" | "Missing" | "FileWithoutEvidence";
/** Two actor ids that must differ (the two-person rule); each is non-blank and at most 128 bytes. */
export type TwoPeople = [string, string];
export type EvidenceEvent =
  | { kind: "PutSucceeded" } | { kind: "PutFailed" } | { kind: "ObjectMissing" }
  | { kind: "DecidedWithoutEvidence"; by: TwoPeople };
export type ClearApproval = { kind: "One"; by: string } | { kind: "Two"; by: TwoPeople };
export interface ReportClock { status: ReportStatus; sentAt: Time[]; finishedAt: Time | null }
export type CaseKind = "KnownHash" | "Classifier" | "RemovalRequest" | "ModeratorSighting";
export interface CaseFile { sha256Hex: string; state: EvidenceState }
export interface LegalHold { setBy: TwoPeople; reason: string; setAt: Time }
export type DestructionBasis = { kind: "LawEnforcementRequest"; requestRef: string } | { kind: "ClearedAndExpired" };
export interface Destruction { basis: DestructionBasis; at: Time; by: TwoPeople | null }
/** A case record, as the host stores it. The module never keeps state: pass it in, store what comes back. */
export interface CaseRecord {
  kind: CaseKind;
  createdAt: Time;
  review: { outcome: "Confirmed" | "FalsePositive"; at: Time } | null;
  files: CaseFile[];
  evidenceHold: LegalHold | null;
  destruction: Destruction | null;
}
export type Ineligible = "Undecided" | "Confirmed" | "BeforeEnd" | "LegalHold" | "SharedFile" | "AlreadyBegun";
export type ReleaseRefusal = "NotCleared" | "EvidenceCopyNotHeld" | "BeforeEnd" | "LegalHold";

// ---- the wrapper --------------------------------------------------------------------------------

export declare class SafeImage {
  /** `source`: the module's bytes, or an already compiled module (a worker host compiles it at deploy time). */
  static create(source: BufferSource | WebAssembly.Module): Promise<SafeImage>;
  readonly abiVersion: number;

  // layer 1
  /** PDQ hash of one RGB frame (3 bytes per pixel, row-major). `hash` is 32 bytes in the layer 1 layout. */
  pdq(rgb: Uint8Array, width: number, height: number): { hash: Uint8Array; quality: number };
  /** PDQ hashes of one frame in all eight orientations. */
  pdqDihedral(rgb: Uint8Array, width: number, height: number): { hashes: Uint8Array[]; quality: number };

  // layer 2
  /** The strongest of several outcomes; an empty list is `Unavailable(NotConfigured)`. */
  strongest(outcomes: MatchOutcome[]): MatchOutcome;
  /** `null` for a 2xx status, otherwise why the provider is unavailable. */
  classifyStatus(status: number): Unavailable | null;
  withinCap(bodyLen: number, cap: number): boolean;
  retryDecide(policy: { maxRetries: number; delayMs: Time }, attemptsMade: number, reason: Unavailable, elapsedMs: Time, deadlineMs: Time): RetryDecision;
  /** Validates a request of 1 to 8 hashes (32 bytes each); returns the count, throws otherwise. */
  validateMatchRequest(hashes: Uint8Array[]): number;

  // layer 4
  reportNew(fileCount: number, credentialsReady: boolean): ReportState;
  reportNextStep(state: ReportState, now: Time): Step;
  reportApply(state: ReportState, step: Step, answer: DestinationAnswer, now: Time): { state: ReportState; event: ReportEvent | null };
  reportSyncCredentials(state: ReportState, ready: boolean): ReportState;
  /** Throws `ALREADY_FINISHED` when the report already finished. */
  reportRequestRetract(state: ReportState): ReportState;
  reportRetryFailed(state: ReportState): ReportState;
  backoffSecs(attempts: number): bigint;
  deletionWindowElapsed(openedAt: Time, lastModifiedAt: Time, now: Time, policy: { openSecs: Time; idleSecs: Time }): boolean;

  // layer 3
  /** Throws `TRANSITION_REFUSED` for an event not valid in `state`. */
  evidenceTransition(state: EvidenceState, event: EvidenceEvent): EvidenceState;
  caseEvidence(states: EvidenceState[]): "Failed" | "Pending" | "Present";
  reportFileDisposition(state: EvidenceState, objectFound: boolean): "SendWithFile" | "SendWithoutFile" | "Hold";
  reportMaySubmit(files: { state: EvidenceState; objectFound: boolean }[]): boolean;
  preservationPeriodSecs(): bigint;
  /** The earliest time evidence may be released; `null` means **keep everything**. */
  preserveUntil(matchAt: Time, decided: boolean, clocks: ReportClock[]): bigint | null;
  caseApplyFileEvent(theCase: CaseRecord, index: number, event: EvidenceEvent): CaseRecord;
  caseConfirm(theCase: CaseRecord, at: Time): CaseRecord;
  caseClear(theCase: CaseRecord, approval: ClearApproval, at: Time): CaseRecord;
  caseSetHold(theCase: CaseRecord, by: TwoPeople, reason: string, at: Time): CaseRecord;
  caseLiftHold(theCase: CaseRecord, by: TwoPeople): CaseRecord;
  /** Records the start of destruction on a law-enforcement request. **Records only; deletes nothing.** */
  caseBeginDestructionOnRequest(theCase: CaseRecord, requestRef: string, by: TwoPeople, at: Time): CaseRecord;
  /** Records the start of destruction of cleared evidence past its date. **Records only.** */
  caseBeginDestructionExpired(theCase: CaseRecord, until: Time | null, now: Time, sharesDigest: boolean): CaseRecord;
  /** `null` when the reaper may begin destroying this case's evidence, else why not. */
  caseReaperEligibility(theCase: CaseRecord, until: Time | null, now: Time, sharesDigest: boolean): Ineligible | null;
  /** `null` when a media hold may be released, else why not. */
  checkReleaseHold(theCase: CaseRecord, target: "ServingKey" | "EvidenceKey", until: Time | null, now: Time, evidenceCopiesHeld: boolean, by: TwoPeople): ReleaseRefusal | null;
}

/** Upstream text form of a hash: 64 hex digits, word 15 first. */
export declare function hashToHex(hash: Uint8Array): string;
export declare function hashFromHex(text: string): Uint8Array;

// ---- encodings (for a host that stores or forwards the messages) ---------------------------------

export declare function encodeOutcome(o: MatchOutcome): Uint8Array;
export declare function decodeOutcome(b: Uint8Array): MatchOutcome;
export declare function encodeOutcomes(list: MatchOutcome[]): Uint8Array;
export declare function decodeOutcomes(b: Uint8Array): MatchOutcome[];
export declare function encodeHashes(hashes: Uint8Array[]): Uint8Array;
export declare function encodeState(s: ReportState): Uint8Array;
export declare function decodeState(b: Uint8Array): ReportState;
export declare function decodeApplied(b: Uint8Array): { state: ReportState; event: ReportEvent | null };
export declare function encodeStep(s: Step): Uint8Array;
export declare function decodeStep(b: Uint8Array): Step;
export declare function encodeAnswer(a: DestinationAnswer): Uint8Array;
export declare function decodeAnswer(b: Uint8Array): DestinationAnswer;
export declare function encodeCase(c: CaseRecord): Uint8Array;
export declare function decodeCase(b: Uint8Array): CaseRecord;
export declare function encodeTwo(p: TwoPeople): Uint8Array;
export declare function decodeTwo(b: Uint8Array): TwoPeople;
export declare function encodeEvidenceEvent(e: EvidenceEvent): Uint8Array;
export declare function decodeEvidenceEvent(b: Uint8Array): EvidenceEvent;
export declare function encodeApproval(a: ClearApproval): Uint8Array;
export declare function decodeApproval(b: Uint8Array): ClearApproval;
export declare function encodeStates(s: EvidenceState[]): Uint8Array;
export declare function decodeStates(b: Uint8Array): EvidenceState[];
export declare function encodeStateFound(l: { state: EvidenceState; objectFound: boolean }[]): Uint8Array;
export declare function decodeStateFound(b: Uint8Array): { state: EvidenceState; objectFound: boolean }[];
export declare function encodeClocks(l: ReportClock[]): Uint8Array;
export declare function decodeClocks(b: Uint8Array): ReportClock[];
