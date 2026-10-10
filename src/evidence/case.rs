use alloc::string::String;
use alloc::vec::Vec;

use super::people::{ActorId, TwoPeople};
use super::state::{EvidenceEvent, EvidenceState, TransitionRefused, transition};

/// What kind of signal opened a case. Assigned by the host (its `KindPolicy`); the names are the
/// first consumer's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaseKind {
    /// A known-hash match.
    KnownHash,
    /// A classifier flag.
    Classifier,
    /// A removal request from the provider.
    RemovalRequest,
    /// A moderator saw it. Confirmed at intake; there is nothing to clear.
    ModeratorSighting,
}

/// A decision. `None` in [`CaseRecord::review`] means undecided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOutcome {
    /// A moderator confirmed it.
    Confirmed,
    /// Cleared as a false positive.
    FalsePositive,
}

/// One file of a case, by digest, with its evidence state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    /// Lowercase hex SHA-256.
    pub sha256_hex: String,
    /// Where its evidence copy is.
    pub state: EvidenceState,
}

/// An evidence legal hold: set and lifted only by two different people.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegalHold {
    /// Who set it.
    pub set_by: TwoPeople,
    /// Why (the legal process that asks us to keep the material).
    pub reason: String,
    /// When (seconds since the Unix epoch).
    pub set_at: u64,
}

/// Why evidence is being destroyed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestructionBasis {
    /// A law-enforcement request to destroy the visual depiction (the statute's own exception).
    /// `request_ref` names the request.
    LawEnforcementRequest {
        /// The request's reference, as the operator records it.
        request_ref: String,
    },
    /// A cleared case past its preservation end with no hold and no shared file.
    ClearedAndExpired,
}

/// That destruction has begun. **The crate records this and has no delete capability anywhere**:
/// the host deletes the bytes. After this is set the case refuses every other write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destruction {
    /// Why.
    pub basis: DestructionBasis,
    /// When it began.
    pub at: u64,
    /// Who authorised it (two people for a law-enforcement request; `None` for the expiry reaper).
    pub by: Option<TwoPeople>,
}

/// The persisted record of one case's evidence and review. Plain data: the host stores it and
/// changes it only through the functions in this module, each of which returns a new record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseRecord {
    /// What opened it.
    pub kind: CaseKind,
    /// When the match happened (seconds since the Unix epoch).
    pub created_at: u64,
    /// The decision and when it was made; `None` is undecided.
    pub review: Option<(ReviewOutcome, u64)>,
    /// The case's files.
    pub files: Vec<FileRecord>,
    /// An evidence legal hold, if one is set.
    pub evidence_hold: Option<LegalHold>,
    /// Set once destruction has begun.
    pub destruction: Option<Destruction>,
}

impl CaseRecord {
    /// A new undecided case.
    pub fn new(kind: CaseKind, created_at: u64, files: Vec<FileRecord>) -> Self {
        CaseRecord {
            kind,
            created_at,
            review: None,
            files,
            evidence_hold: None,
            destruction: None,
        }
    }

    /// The outcome, if decided.
    pub fn outcome(&self) -> Option<ReviewOutcome> {
        self.review.map(|(o, _)| o)
    }
}

/// How a clear was approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClearApproval {
    /// One reviewer.
    One(ActorId),
    /// Two different people.
    Two(TwoPeople),
}

/// Why a case operation was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaseError {
    /// Destruction has begun: the case refuses every other write.
    Destroyed,
    /// The case already has a decision.
    AlreadyDecided,
    /// This kind of case cannot be cleared (a sighting is confirmed at intake).
    NotClearable(CaseKind),
    /// This kind of case needs two different people to clear.
    NeedsTwoPeople(CaseKind),
    /// No file at that index.
    NoSuchFile(usize),
    /// The evidence table does not list that transition.
    Transition(TransitionRefused),
    /// A legal hold is already set.
    HoldAlreadySet,
    /// No legal hold to lift.
    NoHold,
    /// A reason or reference was blank.
    Blank(&'static str),
    /// Destruction has already begun.
    DestructionAlreadyBegun,
    /// The expiry reaper may not act yet.
    NotEligible(Ineligible),
}

/// Why the expiry reaper may not destroy a case's evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ineligible {
    /// Undecided: keep everything.
    Undecided,
    /// A confirmed case's evidence is kept; there is no tool to release it.
    Confirmed,
    /// The preservation clock says keep everything, or is not yet past its end.
    BeforeEnd,
    /// An evidence legal hold applies.
    LegalHold,
    /// Another live or confirmed case holds a file with the same digest.
    SharedFile,
    /// Destruction has already begun.
    AlreadyBegun,
}

fn live(case: &CaseRecord) -> Result<(), CaseError> {
    if case.destruction.is_some() {
        Err(CaseError::Destroyed)
    } else {
        Ok(())
    }
}

/// Applies an evidence event to file `index`.
pub fn apply_file_event(
    case: &CaseRecord,
    index: usize,
    event: &EvidenceEvent,
) -> Result<CaseRecord, CaseError> {
    live(case)?;
    let mut c = case.clone();
    let file = c.files.get_mut(index).ok_or(CaseError::NoSuchFile(index))?;
    file.state = transition(file.state, event).map_err(CaseError::Transition)?;
    Ok(c)
}

/// Records a confirmation. The host checks that every file was viewed.
pub fn confirm(case: &CaseRecord, at: u64) -> Result<CaseRecord, CaseError> {
    live(case)?;
    if case.review.is_some() {
        return Err(CaseError::AlreadyDecided);
    }
    let mut c = case.clone();
    c.review = Some((ReviewOutcome::Confirmed, at));
    Ok(c)
}

/// Records a clear (false positive). A known-hash or removal-request case needs two different
/// people; a classifier case takes one reviewer or two; a sighting cannot be cleared.
pub fn clear(
    case: &CaseRecord,
    approval: &ClearApproval,
    at: u64,
) -> Result<CaseRecord, CaseError> {
    live(case)?;
    if case.review.is_some() {
        return Err(CaseError::AlreadyDecided);
    }
    match case.kind {
        CaseKind::ModeratorSighting => return Err(CaseError::NotClearable(case.kind)),
        CaseKind::KnownHash | CaseKind::RemovalRequest => {
            if !matches!(approval, ClearApproval::Two(_)) {
                return Err(CaseError::NeedsTwoPeople(case.kind));
            }
        }
        CaseKind::Classifier => {}
    }
    let mut c = case.clone();
    c.review = Some((ReviewOutcome::FalsePositive, at));
    Ok(c)
}

/// Sets an evidence legal hold. Two different people, with a reason; refused once destruction has
/// begun.
pub fn set_evidence_legal_hold(
    case: &CaseRecord,
    by: &TwoPeople,
    reason: &str,
    at: u64,
) -> Result<CaseRecord, CaseError> {
    live(case)?;
    if reason.trim().is_empty() {
        return Err(CaseError::Blank("reason"));
    }
    if case.evidence_hold.is_some() {
        return Err(CaseError::HoldAlreadySet);
    }
    let mut c = case.clone();
    c.evidence_hold = Some(LegalHold {
        set_by: by.clone(),
        reason: reason.into(),
        set_at: at,
    });
    Ok(c)
}

/// Lifts an evidence legal hold. Two different people.
pub fn lift_evidence_legal_hold(
    case: &CaseRecord,
    _by: &TwoPeople,
) -> Result<CaseRecord, CaseError> {
    live(case)?;
    if case.evidence_hold.is_none() {
        return Err(CaseError::NoHold);
    }
    let mut c = case.clone();
    c.evidence_hold = None;
    Ok(c)
}

/// Records the start of destruction on a law-enforcement request: two different people, with the
/// request's reference. This bypasses the hold and the clock, as the statute's exception does, and
/// is recorded so it can be audited. The host deletes the bytes; this crate cannot.
pub fn begin_destruction_on_request(
    case: &CaseRecord,
    request_ref: &str,
    by: &TwoPeople,
    at: u64,
) -> Result<CaseRecord, CaseError> {
    if case.destruction.is_some() {
        return Err(CaseError::DestructionAlreadyBegun);
    }
    if request_ref.trim().is_empty() {
        return Err(CaseError::Blank("request_ref"));
    }
    let mut c = case.clone();
    c.destruction = Some(Destruction {
        basis: DestructionBasis::LawEnforcementRequest {
            request_ref: request_ref.into(),
        },
        at,
        by: Some(by.clone()),
    });
    Ok(c)
}

/// Whether the expiry reaper may begin destroying this case's evidence. Only a **cleared** case
/// past `preserve_until` with no evidence legal hold and no file shared with a live or confirmed
/// case is eligible; a confirmed case's evidence is kept.
pub fn reaper_eligibility(
    case: &CaseRecord,
    preserve_until: Option<u64>,
    now: u64,
    shares_digest_with_live_case: bool,
) -> Result<(), Ineligible> {
    if case.destruction.is_some() {
        return Err(Ineligible::AlreadyBegun);
    }
    match case.outcome() {
        None => return Err(Ineligible::Undecided),
        Some(ReviewOutcome::Confirmed) => return Err(Ineligible::Confirmed),
        Some(ReviewOutcome::FalsePositive) => {}
    }
    match preserve_until {
        Some(end) if now > end => {}
        _ => return Err(Ineligible::BeforeEnd),
    }
    if case.evidence_hold.is_some() {
        return Err(Ineligible::LegalHold);
    }
    if shares_digest_with_live_case {
        return Err(Ineligible::SharedFile);
    }
    Ok(())
}

/// Records the start of destruction by the expiry reaper, if [`reaper_eligibility`] allows it.
pub fn begin_destruction_expired(
    case: &CaseRecord,
    preserve_until: Option<u64>,
    now: u64,
    shares_digest_with_live_case: bool,
) -> Result<CaseRecord, CaseError> {
    reaper_eligibility(case, preserve_until, now, shares_digest_with_live_case)
        .map_err(CaseError::NotEligible)?;
    let mut c = case.clone();
    c.destruction = Some(Destruction {
        basis: DestructionBasis::ClearedAndExpired,
        at: now,
        by: None,
    });
    Ok(c)
}

/// Which object a media hold is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldTarget {
    /// The key the public copy was served from.
    ServingKey,
    /// The preserved evidence copy.
    EvidenceKey,
}

/// Why a hold may not be released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseRefusal {
    /// The case is not cleared (undecided or confirmed).
    NotCleared,
    /// The evidence copy is not yet held and verified, so the serving hold must stay.
    EvidenceCopyNotHeld,
    /// An evidence hold may not be released before the preservation end, even by two people.
    BeforeEnd,
    /// An evidence legal hold applies.
    LegalHold,
}

/// Whether a media hold of the CSAM category may be released. Takes [`TwoPeople`] so a release is
/// always a two-person act; it is **not** enough on its own:
///
/// - a **serving-key** hold is released only on a cleared case, and only once the evidence copies
///   are held and verified (`evidence_copies_held`), so the public copy can return while the
///   preserved copy stays;
/// - an **evidence-key** hold is released only on a cleared case, strictly after `preserve_until`,
///   with no evidence legal hold; never before the end, even by two people.
pub fn check_release_hold(
    case: &CaseRecord,
    target: HoldTarget,
    preserve_until: Option<u64>,
    now: u64,
    evidence_copies_held: bool,
    _by: &TwoPeople,
) -> Result<(), ReleaseRefusal> {
    if case.outcome() != Some(ReviewOutcome::FalsePositive) {
        return Err(ReleaseRefusal::NotCleared);
    }
    match target {
        HoldTarget::ServingKey => {
            if evidence_copies_held {
                Ok(())
            } else {
                Err(ReleaseRefusal::EvidenceCopyNotHeld)
            }
        }
        HoldTarget::EvidenceKey => {
            match preserve_until {
                Some(end) if now > end => {}
                _ => return Err(ReleaseRefusal::BeforeEnd),
            }
            if case.evidence_hold.is_some() {
                return Err(ReleaseRefusal::LegalHold);
            }
            Ok(())
        }
    }
}
