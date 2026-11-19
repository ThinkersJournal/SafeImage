// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;

use super::case::{CaseRecord, ReviewOutcome};
use super::people::ActorId;

/// Why layer 3 holds match data (design invariant I8). A storage adapter enumerates by this to
/// find what it may delete: nothing is held for a reason not named here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RetentionPurpose {
    /// Kept so the decision and the match can be audited after the evidence itself may go.
    Audit,
    /// Kept because law or a report requires the evidence to be preserved.
    LegalPreservation,
}

/// The purposes a case's data is held for at `now`, derived from its record; there is no setter,
/// so a tag cannot disagree with the record.
///
/// - [`RetentionPurpose::LegalPreservation`]: the case is confirmed, or has an evidence legal
///   hold, or `preserve_until` is `None` (keep everything) or not yet passed.
/// - [`RetentionPurpose::Audit`]: nothing else holds the case and no destruction has begun; the
///   record of the decision stays for audit.
/// - Empty: destruction has begun and nothing legal remains.
///
/// Sorted, without duplicates. `preserve_until` is [`super::preserve_until`]'s answer.
pub fn retention_purposes(
    case: &CaseRecord,
    preserve_until: Option<u64>,
    now: u64,
) -> Vec<RetentionPurpose> {
    let legal = case.evidence_hold.is_some()
        || case.outcome() == Some(ReviewOutcome::Confirmed)
        || preserve_until.is_none_or(|end| now <= end);
    let mut out = Vec::new();
    if legal {
        out.push(RetentionPurpose::LegalPreservation);
    } else if case.destruction.is_none() {
        out.push(RetentionPurpose::Audit);
    }
    out
}

/// Why a file counts as viewed by the operator's service (the ESP): never a button press, only
/// one of these two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewBasis {
    /// The file was served to a reviewer under a grant belonging to this case.
    ServedUnderGrant {
        /// The grant's id, as the host issued it.
        grant_id: String,
    },
    /// A named person attests they saw the whole file.
    AttestedSighting {
        /// Who attests.
        by: ActorId,
    },
}

/// One recorded sign that a file was viewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewEvidence {
    /// Lowercase hex SHA-256 of the file.
    pub sha256_hex: String,
    /// Why it counts.
    pub basis: ViewBasis,
    /// Seconds since the Unix epoch.
    pub at: u64,
}

/// Whether `sha256_hex` was viewed by the ESP: there is [`ViewEvidence`] for that file that is
/// either an attested sighting or a service under one of `case_grant_ids`. A grant belonging to
/// another case does not count. Feeds `ReportFile::viewed_by_operator`.
pub fn viewed_by_esp(
    sha256_hex: &str,
    case_grant_ids: &[String],
    evidence: &[ViewEvidence],
) -> bool {
    evidence.iter().any(|e| {
        e.sha256_hex == sha256_hex
            && match &e.basis {
                ViewBasis::AttestedSighting { .. } => true,
                ViewBasis::ServedUnderGrant { grant_id } => case_grant_ids.contains(grant_id),
            }
    })
}
