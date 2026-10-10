//! Evidence states of a case file, copied from the first consumer's upload-scan design
//! section 5.7 (not re-derived): five states, eight transitions, and "no other transition exists".

use super::people::TwoPeople;

/// The state of one case file's evidence copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceState {
    /// The evidence write has not completed yet.
    Pending,
    /// The evidence object exists.
    Present,
    /// The write failed (inline retries exhausted, or still pending after the tick's grace).
    Failed,
    /// The object was present and the daily check found it gone.
    Missing,
    /// Two different people decided to file without the evidence.
    FileWithoutEvidence,
}

/// What happened to a file's evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceEvent {
    /// The put and the `head` succeeded (the upload request, or a later upload of the same bytes).
    PutSucceeded,
    /// Bounded inline retries were exhausted, or a tick found the file still pending after its
    /// grace period.
    PutFailed,
    /// The daily check's `head` found the object gone.
    ObjectMissing,
    /// Two different people decided to file without it.
    DecidedWithoutEvidence(TwoPeople),
}

/// A pair the table does not list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionRefused {
    /// The state it was in.
    pub from: EvidenceState,
}

/// The next state, or a refusal. The table:
///
/// | From | Event | To |
/// |---|---|---|
/// | `Pending` | `PutSucceeded` | `Present` |
/// | `Pending` | `PutFailed` | `Failed` |
/// | `Present` | `ObjectMissing` | `Missing` |
/// | `Failed` or `Missing` | `PutSucceeded` | `Present` |
/// | `Failed` or `Missing` | `DecidedWithoutEvidence` | `FileWithoutEvidence` |
/// | `FileWithoutEvidence` | `PutSucceeded` | `Present` |
///
/// No other transition exists. Destruction of a case's evidence is **not** a state change here.
pub fn transition(
    from: EvidenceState,
    event: &EvidenceEvent,
) -> Result<EvidenceState, TransitionRefused> {
    use EvidenceEvent::*;
    use EvidenceState::*;
    match (from, event) {
        (Pending, PutSucceeded) => Ok(Present),
        (Pending, PutFailed) => Ok(Failed),
        (Present, ObjectMissing) => Ok(Missing),
        (Failed | Missing, PutSucceeded) => Ok(Present),
        (Failed | Missing, DecidedWithoutEvidence(_)) => Ok(FileWithoutEvidence),
        (FileWithoutEvidence, PutSucceeded) => Ok(Present),
        _ => Err(TransitionRefused { from }),
    }
}

/// A case's evidence state, derived from its files and never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseEvidence {
    /// Some file is `Failed` or `Missing`.
    Failed,
    /// No file failed, and some file is `Pending`.
    Pending,
    /// Otherwise (including a case with no files).
    Present,
}

/// Derives the case's evidence state: failed or missing beats pending beats present.
pub fn case_evidence(files: &[EvidenceState]) -> CaseEvidence {
    if files
        .iter()
        .any(|s| matches!(s, EvidenceState::Failed | EvidenceState::Missing))
    {
        CaseEvidence::Failed
    } else if files.iter().any(|s| matches!(s, EvidenceState::Pending)) {
        CaseEvidence::Pending
    } else {
        CaseEvidence::Present
    }
}

/// What the drain may do with one report file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDisposition {
    /// Send it with its bytes.
    SendWithFile,
    /// Send the report without this file (two people decided).
    SendWithoutFile,
    /// Hold the report: no destination call until the bytes arrive or two people decide.
    Hold,
}

/// Upload-scan section 5.7's `reportFileDisposition`: a `Present` file whose object `head` finds
/// is sent with its bytes; a `FileWithoutEvidence` file is sent without; everything else holds
/// (a `Present` file whose object is not found holds, and does not change state here: the next
/// daily check does).
pub fn report_file_disposition(state: EvidenceState, object_found: bool) -> FileDisposition {
    match state {
        EvidenceState::Present if object_found => FileDisposition::SendWithFile,
        EvidenceState::Present => FileDisposition::Hold,
        EvidenceState::FileWithoutEvidence => FileDisposition::SendWithoutFile,
        EvidenceState::Pending | EvidenceState::Failed | EvidenceState::Missing => {
            FileDisposition::Hold
        }
    }
}

/// A report may be submitted only when it has files and none of them holds it.
pub fn report_may_submit(files: &[(EvidenceState, bool)]) -> bool {
    !files.is_empty()
        && files
            .iter()
            .all(|(s, found)| report_file_disposition(*s, *found) != FileDisposition::Hold)
}
