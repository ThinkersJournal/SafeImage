//! Layer 3: evidence states, review rules, preservation clock and the two-person set, as plain
//! records and pure functions.
//!
//! **No storage is assumed and nothing is persisted here.** The host stores a [`CaseRecord`] and
//! each function returns a new one; the host writes it with a compare-and-set on what it read.
//! There is **no delete capability** anywhere in the crate: destruction is *recorded*
//! ([`Destruction`]) and the host deletes bytes. The names and the evidence table follow the first
//! consumer's written design (upload-scan section 5.7 and the rev-3 pipeline spec); do not assume
//! the uploader is the media owner, since after anonymisation an owner is a bare id.
//!
//! The two-person set (design invariant I9) is: clearing a known-hash case, clearing a
//! removal-request case, deciding to file without evidence, setting and lifting an evidence legal
//! hold, and destruction on a law-enforcement request. Each takes a [`TwoPeople`] (or a
//! [`ClearApproval`] that must contain one), and [`TwoPeople::new`] refuses equal ids.

mod case;
mod people;
mod preserve;
mod state;

pub use case::{
    CaseError, CaseKind, CaseRecord, ClearApproval, Destruction, DestructionBasis, FileRecord,
    HoldTarget, Ineligible, LegalHold, ReleaseRefusal, ReviewOutcome, apply_file_event,
    begin_destruction_expired, begin_destruction_on_request, check_release_hold, clear, confirm,
    lift_evidence_legal_hold, reaper_eligibility, set_evidence_legal_hold,
};
pub use people::{ActorId, SamePerson, TwoPeople};
pub use preserve::{ReportClock, preservation_period_secs, preserve_until};
pub use state::{
    CaseEvidence, EvidenceEvent, EvidenceState, FileDisposition, TransitionRefused, case_evidence,
    report_file_disposition, report_may_submit, transition,
};
