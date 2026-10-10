use alloc::vec::Vec;

use crate::report::Status as ReportStatus;

/// One year, as the crate counts it: **366 days in seconds**.
///
/// This applies the first consumer's existing rule that, where the statute is unclear, the design
/// takes the reading that keeps evidence longer; it is not a new legal number. A later ruling
/// changes this one function.
pub fn preservation_period_secs() -> u64 {
    366 * 24 * 60 * 60
}

/// What the preservation clock needs to know about one report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportClock {
    /// The report's persisted status.
    pub status: ReportStatus,
    /// Every time a send was made (each send is its own submission, including one whose answer
    /// was lost and one later retracted).
    pub sent_at: Vec<u64>,
    /// When the report was finished, if it was.
    pub finished_at: Option<u64>,
}

/// Whether a report is in a status from which it could still be sent.
fn could_still_be_sent(s: ReportStatus) -> bool {
    matches!(
        s,
        ReportStatus::AwaitingCredentials
            | ReportStatus::Pending
            | ReportStatus::Submitted
            | ReportStatus::RetractPending
            | ReportStatus::Failed
    )
}

/// The earliest time evidence may be released, in seconds since the Unix epoch, or `None` meaning
/// **keep everything** (the case is undecided, or a report could still be sent).
///
/// Otherwise it is one preservation period after the latest of: the match, every send, every
/// finish. A deletion-window resubmission or a late report moves the end later, never earlier.
pub fn preserve_until(match_at: u64, decided: bool, reports: &[ReportClock]) -> Option<u64> {
    if !decided || reports.iter().any(|r| could_still_be_sent(r.status)) {
        return None;
    }
    let mut latest = match_at;
    for r in reports {
        for &t in &r.sent_at {
            latest = latest.max(t);
        }
        if let Some(t) = r.finished_at {
            latest = latest.max(t);
        }
    }
    Some(latest.saturating_add(preservation_period_secs()))
}
