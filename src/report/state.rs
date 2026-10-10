use alloc::string::String;
use alloc::vec::Vec;

use super::destination::DestinationAnswer;

/// Longest detail string kept from an answer (bytes).
const MAX_NOTE_BYTES: usize = 256;

fn capped(mut s: String) -> String {
    if s.len() > MAX_NOTE_BYTES {
        let mut end = MAX_NOTE_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// Where a report is. The persisted statuses of the drain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Status {
    /// A required credential is missing. Nothing is sent; alarm.
    AwaitingCredentials,
    /// Ready to send.
    Pending,
    /// The destination has the report and its id is recorded; files are being sent.
    Submitted,
    /// Done.
    Finished,
    /// Terminal until a human acts: the destination rejected the document.
    Failed,
    /// A withdrawal is queued for a submitted report.
    RetractPending,
    /// Withdrawn (or cancelled before it was ever sent).
    Withdrawn,
}

/// Which files of a report have been sent and described.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileProgress {
    /// The file's bytes were accepted.
    pub uploaded: bool,
    /// The file's metadata was accepted.
    pub described: bool,
}

/// How a report finished, when it did not finish by our own `Finish` step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinishNote {
    /// The destination said it was already finished (an earlier answer was lost). Success.
    AlreadyFinished,
    /// A withdrawal found it already finished. Flag for the runbook; nothing preserved is released.
    FinishedBeforeRetract,
}

/// The persisted record of one report. Plain data: the host stores and reloads it, and changes it
/// only through [`apply`], [`sync_credentials`], [`request_retract`] and [`retry_failed`]. A host
/// should write each transition with a compare-and-set on the status it read, so a concurrent
/// withdrawal wins over a send whose answer arrives late.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportState {
    /// Where the report is.
    pub status: Status,
    /// Consecutive failed attempts at the current step.
    pub attempts: u32,
    /// Do not act before this time (seconds since the Unix epoch), if set.
    pub next_attempt_at: Option<u64>,
    /// The destination's identifier for the report, once submitted.
    pub report_id: Option<String>,
    /// One entry per file, in report order.
    pub files: Vec<FileProgress>,
    /// The last code or note the destination gave, for the case file (capped).
    pub last_code: Option<String>,
    /// Report ids the destination discarded (deletion window); kept for the record.
    pub abandoned_ids: Vec<String>,
    /// Set when the report finished by an unusual route.
    pub finish_note: Option<FinishNote>,
}

impl ReportState {
    /// A new report with `file_count` files. `credentials_ready` is whether every required
    /// credential slot is filled.
    pub fn new(file_count: usize, credentials_ready: bool) -> Self {
        ReportState {
            status: if credentials_ready {
                Status::Pending
            } else {
                Status::AwaitingCredentials
            },
            attempts: 0,
            next_attempt_at: None,
            report_id: None,
            files: alloc::vec![FileProgress::default(); file_count],
            last_code: None,
            abandoned_ids: Vec::new(),
            finish_note: None,
        }
    }
}

/// Why nothing can be done now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldReason {
    /// A required credential is missing.
    AwaitingCredentials,
    /// The destination rejected the document; a human must act.
    Failed,
}

/// What is due next. The host performs it and calls [`apply`] with the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Send the document.
    Submit,
    /// Send file `file`'s bytes.
    Upload {
        /// Index into [`ReportState::files`].
        file: usize,
    },
    /// Send file `file`'s metadata.
    Describe {
        /// Index into [`ReportState::files`].
        file: usize,
    },
    /// Mark the report complete.
    Finish,
    /// Withdraw the submitted report.
    Retract,
    /// Nothing before this time.
    Wait {
        /// Seconds since the Unix epoch.
        until: u64,
    },
    /// Nothing can be done until something outside changes.
    Hold(HoldReason),
    /// Finished or withdrawn.
    Done,
}

/// Something the host must act on (alarm, case note) that is not a state change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The destination refused our credentials. Raise at once.
    AuthRefused,
    /// The destination rejected the document (detail capped).
    Invalid(String),
    /// The destination discarded the unfinished report; it goes back to `Pending` to be sent anew.
    WindowExpired,
    /// A code the profile does not map arrived; handled as transient, never as success.
    UnknownAnswer(String),
    /// A withdrawal found the report already finished. Flag for the runbook.
    FinishedBeforeRetract,
    /// A submit was "accepted" without the destination's report id; handled as transient.
    MissingReportId,
    /// The step does not apply in the report's current status; nothing changed.
    StepNotValidHere,
}

/// Seconds to wait after `attempts` consecutive failures: 2, 4, 8, 16 minutes, then 30 minutes.
pub fn backoff_secs(attempts: u32) -> u64 {
    const CAP: u64 = 30 * 60;
    let n = attempts.max(1) - 1;
    if n >= 5 {
        return CAP;
    }
    ((2u64 * 60) << n).min(CAP)
}

/// What is due next for `state` at time `now`. Pure.
pub fn next_step(state: &ReportState, now: u64) -> Step {
    match state.status {
        Status::AwaitingCredentials => return Step::Hold(HoldReason::AwaitingCredentials),
        Status::Failed => return Step::Hold(HoldReason::Failed),
        Status::Finished | Status::Withdrawn => return Step::Done,
        _ => {}
    }
    if let Some(t) = state.next_attempt_at
        && t > now
    {
        return Step::Wait { until: t };
    }
    match state.status {
        Status::Pending => Step::Submit,
        Status::RetractPending => Step::Retract,
        Status::Submitted => {
            for (i, f) in state.files.iter().enumerate() {
                if !f.uploaded {
                    return Step::Upload { file: i };
                }
                if !f.described {
                    return Step::Describe { file: i };
                }
            }
            Step::Finish
        }
        // Handled above.
        Status::AwaitingCredentials | Status::Failed | Status::Finished | Status::Withdrawn => {
            Step::Done
        }
    }
}

fn step_valid(state: &ReportState, step: &Step) -> bool {
    match step {
        Step::Submit => state.status == Status::Pending,
        Step::Upload { file } | Step::Describe { file } => {
            state.status == Status::Submitted && *file < state.files.len()
        }
        Step::Finish => state.status == Status::Submitted,
        Step::Retract => state.status == Status::RetractPending,
        Step::Wait { .. } | Step::Hold(_) | Step::Done => false,
    }
}

fn transient(mut s: ReportState, now: u64, code: Option<String>) -> ReportState {
    s.attempts = s.attempts.saturating_add(1);
    s.next_attempt_at = Some(now.saturating_add(backoff_secs(s.attempts)));
    if let Some(c) = code {
        s.last_code = Some(capped(c));
    }
    s
}

fn success(mut s: ReportState) -> ReportState {
    s.attempts = 0;
    s.next_attempt_at = None;
    s
}

/// Applies the destination's answer to `step`. Pure: returns the new state and, when the host must
/// act, an [`Event`]. `Unknown` and `Transient` answers never advance the report.
pub fn apply(
    state: &ReportState,
    step: &Step,
    answer: &DestinationAnswer,
    now: u64,
) -> (ReportState, Option<Event>) {
    if !step_valid(state, step) {
        return (state.clone(), Some(Event::StepNotValidHere));
    }
    let mut s = state.clone();
    match answer {
        DestinationAnswer::Transient => (transient(s, now, Some("transient".into())), None),
        DestinationAnswer::Unknown(code) => (
            transient(s, now, Some(code.clone())),
            Some(Event::UnknownAnswer(capped(code.clone()))),
        ),
        DestinationAnswer::AuthRefused => (
            transient(s, now, Some("auth_refused".into())),
            Some(Event::AuthRefused),
        ),
        DestinationAnswer::Invalid(detail) => {
            s.status = Status::Failed;
            s.last_code = Some("invalid".into());
            (s, Some(Event::Invalid(capped(detail.clone()))))
        }
        DestinationAnswer::WindowExpired => {
            if let Some(id) = s.report_id.take() {
                s.abandoned_ids.push(id);
            }
            s.last_code = Some("window_expired".into());
            if s.status == Status::RetractPending {
                // The report is gone at the destination: the withdrawal has nothing left to do.
                s.status = Status::Withdrawn;
            } else {
                for f in &mut s.files {
                    *f = FileProgress::default();
                }
                s.status = Status::Pending;
            }
            (success(s), Some(Event::WindowExpired))
        }
        DestinationAnswer::AlreadyFinished => match step {
            Step::Finish => {
                s.status = Status::Finished;
                s.finish_note = Some(FinishNote::AlreadyFinished);
                s.last_code = Some("already_finished".into());
                (success(s), None)
            }
            Step::Retract => {
                s.status = Status::Finished;
                s.finish_note = Some(FinishNote::FinishedBeforeRetract);
                s.last_code = Some("already_finished".into());
                (success(s), Some(Event::FinishedBeforeRetract))
            }
            _ => (
                transient(s, now, Some("already_finished".into())),
                Some(Event::StepNotValidHere),
            ),
        },
        DestinationAnswer::Accepted { report_id } => match step {
            Step::Submit => match report_id {
                Some(id) if !id.trim().is_empty() => {
                    s.report_id = Some(id.clone());
                    s.status = Status::Submitted;
                    (success(s), None)
                }
                _ => (
                    transient(s, now, Some("missing_report_id".into())),
                    Some(Event::MissingReportId),
                ),
            },
            Step::Upload { file } => {
                s.files[*file].uploaded = true;
                (success(s), None)
            }
            Step::Describe { file } => {
                s.files[*file].described = true;
                (success(s), None)
            }
            Step::Finish => {
                s.status = Status::Finished;
                (success(s), None)
            }
            Step::Retract => {
                s.status = Status::Withdrawn;
                (success(s), None)
            }
            Step::Wait { .. } | Step::Hold(_) | Step::Done => (s, Some(Event::StepNotValidHere)),
        },
    }
}

/// Moves a report between `AwaitingCredentials` and `Pending` as credentials appear or vanish.
/// Only those two statuses change; a submitted report is never demoted.
pub fn sync_credentials(state: &ReportState, credentials_ready: bool) -> ReportState {
    let mut s = state.clone();
    match (s.status, credentials_ready) {
        (Status::AwaitingCredentials, true) => s.status = Status::Pending,
        (Status::Pending, false) => s.status = Status::AwaitingCredentials,
        _ => {}
    }
    s
}

/// Why a withdrawal cannot be requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetractError {
    /// The report is already finished: a withdrawal no longer applies.
    AlreadyFinished,
}

/// Requests a withdrawal. A report never sent becomes `Withdrawn` at once (nothing to retract);
/// a submitted one becomes `RetractPending`. **A withdrawal never shortens preservation**; that
/// clock is the evidence layer's.
pub fn request_retract(state: &ReportState) -> Result<ReportState, RetractError> {
    let mut s = state.clone();
    match s.status {
        Status::Finished => Err(RetractError::AlreadyFinished),
        Status::Withdrawn | Status::RetractPending => Ok(s),
        Status::AwaitingCredentials | Status::Pending => {
            s.status = Status::Withdrawn;
            Ok(s)
        }
        Status::Failed | Status::Submitted => {
            s.status = if s.report_id.is_some() {
                Status::RetractPending
            } else {
                Status::Withdrawn
            };
            Ok(s)
        }
    }
}

/// A human's retry of a `Failed` report, after the cause was fixed: back to `Pending` with the
/// per-file progress cleared and any destination id recorded as abandoned. Not a state the
/// drain reaches on its own.
pub fn retry_failed(state: &ReportState) -> ReportState {
    let mut s = state.clone();
    if s.status == Status::Failed {
        if let Some(id) = s.report_id.take() {
            s.abandoned_ids.push(id);
        }
        for f in &mut s.files {
            *f = FileProgress::default();
        }
        s.status = Status::Pending;
        s.attempts = 0;
        s.next_attempt_at = None;
    }
    s
}

/// A destination's deletion-window numbers, supplied by its profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPolicy {
    /// Seconds after the report was opened.
    pub open_secs: u64,
    /// Seconds after its last modification.
    pub idle_secs: u64,
}

/// True when an unfinished report may have been discarded by the destination: `now` is past the
/// later of `opened_at + open_secs` and `last_modified_at + idle_secs`.
pub fn deletion_window_elapsed(
    opened_at: u64,
    last_modified_at: u64,
    now: u64,
    policy: WindowPolicy,
) -> bool {
    let a = opened_at.saturating_add(policy.open_secs);
    let b = last_modified_at.saturating_add(policy.idle_secs);
    now > a.max(b)
}
