// SPDX-License-Identifier: MIT OR Apache-2.0
//! Layer 3: evidence states, review rules, the preservation clock and the two-person set.

use safeimage::evidence::*;
use safeimage::report::Status as ReportStatus;

const DAY: u64 = 86_400;
const YEAR: u64 = 366 * DAY;

fn actor(s: &str) -> ActorId {
    ActorId::new(s).unwrap()
}

fn two() -> TwoPeople {
    TwoPeople::new(actor("admin-a"), actor("admin-b")).unwrap()
}

fn file(state: EvidenceState) -> FileRecord {
    FileRecord {
        sha256_hex: "ab".repeat(32),
        state,
    }
}

fn case(kind: CaseKind) -> CaseRecord {
    CaseRecord::new(kind, 1_000, vec![file(EvidenceState::Present)])
}

fn cleared(kind: CaseKind) -> CaseRecord {
    let approval = ClearApproval::Two(two());
    clear(&case(kind), &approval, 2_000).unwrap()
}

const STATES: [EvidenceState; 5] = [
    EvidenceState::Pending,
    EvidenceState::Present,
    EvidenceState::Failed,
    EvidenceState::Missing,
    EvidenceState::FileWithoutEvidence,
];

fn events() -> Vec<EvidenceEvent> {
    vec![
        EvidenceEvent::PutSucceeded,
        EvidenceEvent::PutFailed,
        EvidenceEvent::ObjectMissing,
        EvidenceEvent::DecidedWithoutEvidence(two()),
    ]
}

// ---- the evidence table ------------------------------------------------------------------------

#[test]
fn the_table_has_exactly_the_documented_transitions_and_no_other() {
    use EvidenceState::*;
    // (from, event index in `events()`, to): section 5.7, row by row.
    let allowed: [(EvidenceState, usize, EvidenceState); 8] = [
        (Pending, 0, Present),
        (Pending, 1, Failed),
        (Present, 2, Missing),
        (Failed, 0, Present),
        (Missing, 0, Present),
        (Failed, 3, FileWithoutEvidence),
        (Missing, 3, FileWithoutEvidence),
        (FileWithoutEvidence, 0, Present),
    ];
    let evs = events();
    let mut allowed_seen = 0;
    for from in STATES {
        for (i, ev) in evs.iter().enumerate() {
            let want = allowed
                .iter()
                .find(|(f, e, _)| *f == from && *e == i)
                .map(|(_, _, t)| *t);
            match (transition(from, ev), want) {
                (Ok(got), Some(w)) => {
                    assert_eq!(got, w, "{from:?} + event {i}");
                    allowed_seen += 1;
                }
                (Err(r), None) => assert_eq!(r.from, from),
                (got, want) => panic!("{from:?} + event {i}: got {got:?}, table says {want:?}"),
            }
        }
    }
    assert_eq!(allowed_seen, 8);
    assert_eq!(STATES.len() * evs.len(), 20);
}

#[test]
fn the_case_evidence_state_is_derived_failed_over_pending_over_present() {
    use EvidenceState::*;
    assert_eq!(case_evidence(&[]), CaseEvidence::Present);
    assert_eq!(
        case_evidence(&[Present, FileWithoutEvidence]),
        CaseEvidence::Present
    );
    assert_eq!(case_evidence(&[Present, Pending]), CaseEvidence::Pending);
    assert_eq!(
        case_evidence(&[Pending, Failed, Present]),
        CaseEvidence::Failed
    );
    assert_eq!(case_evidence(&[Present, Missing]), CaseEvidence::Failed);
    assert_eq!(case_evidence(&[Pending, Missing]), CaseEvidence::Failed);
}

#[test]
fn a_report_is_held_for_any_file_that_is_not_present_and_found_or_decided() {
    use EvidenceState::*;
    use FileDisposition::*;
    assert_eq!(report_file_disposition(Present, true), SendWithFile);
    assert_eq!(report_file_disposition(Present, false), Hold);
    assert_eq!(
        report_file_disposition(FileWithoutEvidence, false),
        SendWithoutFile
    );
    assert_eq!(
        report_file_disposition(FileWithoutEvidence, true),
        SendWithoutFile
    );
    for s in [Pending, Failed, Missing] {
        for found in [true, false] {
            assert_eq!(report_file_disposition(s, found), Hold, "{s:?}");
        }
    }
    assert!(!report_may_submit(&[]), "no files: nothing to send");
    assert!(report_may_submit(&[
        (Present, true),
        (FileWithoutEvidence, false)
    ]));
    assert!(!report_may_submit(&[(Present, true), (Pending, true)]));
    assert!(!report_may_submit(&[(Present, false)]));
}

// ---- two people ----------------------------------------------------------------------------------

#[test]
fn two_people_must_be_different_and_ids_must_be_real() {
    assert_eq!(
        TwoPeople::new(actor("x"), actor("x")).unwrap_err(),
        SamePerson
    );
    let t = TwoPeople::new(actor("x"), actor("y")).unwrap();
    assert_eq!((t.first().as_str(), t.second().as_str()), ("x", "y"));
    assert!(ActorId::new("").is_none());
    assert!(ActorId::new("   ").is_none());
    assert!(ActorId::new("x".repeat(129)).is_none());
    assert!(ActorId::new("x".repeat(128)).is_some());
}

// ---- preservation clock --------------------------------------------------------------------------

fn rc(status: ReportStatus, sent: &[u64], fin: Option<u64>) -> ReportClock {
    ReportClock {
        status,
        sent_at: sent.to_vec(),
        finished_at: fin,
    }
}

#[test]
fn the_period_is_366_days_in_seconds() {
    assert_eq!(preservation_period_secs(), 31_622_400);
    assert_eq!(preservation_period_secs(), YEAR);
}

#[test]
fn undecided_or_still_sendable_means_keep_everything() {
    assert_eq!(preserve_until(1_000, false, &[]), None);
    assert_eq!(
        preserve_until(
            1_000,
            false,
            &[rc(ReportStatus::Finished, &[2_000], Some(3_000))]
        ),
        None
    );
    for s in [
        ReportStatus::AwaitingCredentials,
        ReportStatus::Pending,
        ReportStatus::Submitted,
        ReportStatus::RetractPending,
        ReportStatus::Failed,
    ] {
        assert_eq!(
            preserve_until(1_000, true, &[rc(s, &[], None)]),
            None,
            "{s:?}"
        );
    }
}

#[test]
fn a_decided_case_with_no_send_keeps_one_period_from_the_match() {
    assert_eq!(preserve_until(1_000, true, &[]), Some(1_000 + YEAR));
    assert_eq!(
        preserve_until(1_000, true, &[rc(ReportStatus::Withdrawn, &[], None)]),
        Some(1_000 + YEAR)
    );
}

#[test]
fn every_send_and_every_finish_counts_and_the_latest_wins() {
    // Sends at 5_000 and 9_000 (the second one's answer was lost, then withdrawn); finish at 7_000.
    let r = rc(ReportStatus::Withdrawn, &[5_000, 9_000], Some(7_000));
    assert_eq!(preserve_until(1_000, true, &[r]), Some(9_000 + YEAR));
    // A finish later than every send.
    let r = rc(ReportStatus::Finished, &[5_000], Some(12_000));
    assert_eq!(preserve_until(1_000, true, &[r]), Some(12_000 + YEAR));
    // Two reports: the later of all.
    let a = rc(ReportStatus::Finished, &[2_000], Some(3_000));
    let b = rc(ReportStatus::Withdrawn, &[20_000], None);
    assert_eq!(preserve_until(1_000, true, &[a, b]), Some(20_000 + YEAR));
}

#[test]
fn adding_an_event_never_moves_the_end_earlier() {
    let mut reports = vec![rc(ReportStatus::Finished, &[2_000], Some(3_000))];
    let mut last = preserve_until(1_000, true, &reports).unwrap();
    for t in [500u64, 2_500, 3_000, 4_000, 3_999, 100_000, 99_999] {
        reports[0].sent_at.push(t);
        let now = preserve_until(1_000, true, &reports).unwrap();
        assert!(now >= last, "send at {t} moved the end earlier");
        last = now;
        reports[0].finished_at = Some(reports[0].finished_at.unwrap().max(t));
        let now = preserve_until(1_000, true, &reports).unwrap();
        assert!(now >= last);
        last = now;
    }
    assert_eq!(last, 100_000 + YEAR);
    // A match later than every send still counts (the clock is the latest of the three).
    assert_eq!(
        preserve_until(
            500_000,
            true,
            &[rc(ReportStatus::Finished, &[2_000], Some(3_000))]
        ),
        Some(500_000 + YEAR)
    );
}

// ---- review and the two-person set ---------------------------------------------------------------

#[test]
fn a_known_hash_or_removal_request_clear_needs_two_people_a_classifier_one() {
    for kind in [CaseKind::KnownHash, CaseKind::RemovalRequest] {
        let one = ClearApproval::One(actor("a"));
        assert_eq!(
            clear(&case(kind), &one, 5).unwrap_err(),
            CaseError::NeedsTwoPeople(kind)
        );
        let c = clear(&case(kind), &ClearApproval::Two(two()), 5).unwrap();
        assert_eq!(c.outcome(), Some(ReviewOutcome::FalsePositive));
        assert_eq!(c.review, Some((ReviewOutcome::FalsePositive, 5)));
    }
    let one = ClearApproval::One(actor("a"));
    let c = clear(&case(CaseKind::Classifier), &one, 5).unwrap();
    assert_eq!(c.outcome(), Some(ReviewOutcome::FalsePositive));
    assert!(clear(&case(CaseKind::Classifier), &ClearApproval::Two(two()), 5).is_ok());
}

#[test]
fn a_sighting_cannot_be_cleared_and_a_decided_case_cannot_be_decided_again() {
    assert_eq!(
        clear(
            &case(CaseKind::ModeratorSighting),
            &ClearApproval::Two(two()),
            5
        )
        .unwrap_err(),
        CaseError::NotClearable(CaseKind::ModeratorSighting)
    );
    let confirmed = confirm(&case(CaseKind::KnownHash), 7).unwrap();
    assert_eq!(confirmed.outcome(), Some(ReviewOutcome::Confirmed));
    assert_eq!(
        clear(&confirmed, &ClearApproval::Two(two()), 8).unwrap_err(),
        CaseError::AlreadyDecided
    );
    assert_eq!(
        confirm(&confirmed, 9).unwrap_err(),
        CaseError::AlreadyDecided
    );
    let c = cleared(CaseKind::KnownHash);
    assert_eq!(confirm(&c, 9).unwrap_err(), CaseError::AlreadyDecided);
}

#[test]
fn file_events_follow_the_table_through_the_case() {
    let c = CaseRecord::new(
        CaseKind::KnownHash,
        0,
        vec![file(EvidenceState::Pending), file(EvidenceState::Present)],
    );
    let c = apply_file_event(&c, 0, &EvidenceEvent::PutFailed).unwrap();
    assert_eq!(c.files[0].state, EvidenceState::Failed);
    assert_eq!(c.files[1].state, EvidenceState::Present);
    assert_eq!(
        apply_file_event(&c, 1, &EvidenceEvent::PutFailed).unwrap_err(),
        CaseError::Transition(TransitionRefused {
            from: EvidenceState::Present
        })
    );
    assert_eq!(
        apply_file_event(&c, 5, &EvidenceEvent::PutSucceeded).unwrap_err(),
        CaseError::NoSuchFile(5)
    );
    let c = apply_file_event(&c, 0, &EvidenceEvent::DecidedWithoutEvidence(two())).unwrap();
    assert_eq!(c.files[0].state, EvidenceState::FileWithoutEvidence);
    assert_eq!(
        case_evidence(&c.files.iter().map(|f| f.state).collect::<Vec<_>>()),
        CaseEvidence::Present
    );
}

#[test]
fn legal_holds_are_set_and_lifted_by_two_people_with_a_reason() {
    let c = case(CaseKind::KnownHash);
    assert_eq!(
        set_evidence_legal_hold(&c, &two(), "  ", 1).unwrap_err(),
        CaseError::Blank("reason")
    );
    let held = set_evidence_legal_hold(&c, &two(), "preservation request", 10).unwrap();
    assert_eq!(held.evidence_hold.as_ref().unwrap().set_at, 10);
    assert_eq!(
        set_evidence_legal_hold(&held, &two(), "again", 11).unwrap_err(),
        CaseError::HoldAlreadySet
    );
    assert_eq!(
        lift_evidence_legal_hold(&c, &two()).unwrap_err(),
        CaseError::NoHold
    );
    let lifted = lift_evidence_legal_hold(&held, &two()).unwrap();
    assert!(lifted.evidence_hold.is_none());
}

// ---- destruction ---------------------------------------------------------------------------------

#[test]
fn destruction_is_recorded_and_after_it_every_write_is_refused() {
    let c = case(CaseKind::KnownHash);
    assert_eq!(
        begin_destruction_on_request(&c, "  ", &two(), 5).unwrap_err(),
        CaseError::Blank("request_ref")
    );
    let d = begin_destruction_on_request(&c, "LE-2026-1", &two(), 50).unwrap();
    let rec = d.destruction.clone().unwrap();
    assert_eq!(rec.at, 50);
    assert_eq!(rec.by, Some(two()));
    assert_eq!(
        rec.basis,
        DestructionBasis::LawEnforcementRequest {
            request_ref: "LE-2026-1".into()
        }
    );
    // Every other operation refuses.
    assert_eq!(
        apply_file_event(&d, 0, &EvidenceEvent::ObjectMissing).unwrap_err(),
        CaseError::Destroyed
    );
    assert_eq!(confirm(&d, 1).unwrap_err(), CaseError::Destroyed);
    assert_eq!(
        clear(&d, &ClearApproval::Two(two()), 1).unwrap_err(),
        CaseError::Destroyed
    );
    assert_eq!(
        set_evidence_legal_hold(&d, &two(), "r", 1).unwrap_err(),
        CaseError::Destroyed
    );
    assert_eq!(
        lift_evidence_legal_hold(&d, &two()).unwrap_err(),
        CaseError::Destroyed
    );
    // And it cannot begin twice.
    assert_eq!(
        begin_destruction_on_request(&d, "again", &two(), 60).unwrap_err(),
        CaseError::DestructionAlreadyBegun
    );
    // Destruction changes no evidence state.
    assert_eq!(d.files, c.files);
}

#[test]
fn a_law_enforcement_request_bypasses_the_hold_and_the_clock_but_is_recorded() {
    let c = set_evidence_legal_hold(&case(CaseKind::KnownHash), &two(), "hold", 1).unwrap();
    let d = begin_destruction_on_request(&c, "LE-1", &two(), 2).unwrap();
    assert!(d.destruction.is_some());
}

#[test]
fn the_expiry_reaper_needs_a_cleared_case_past_its_end_with_no_hold_and_no_shared_file() {
    let end = 10_000u64;
    let c = cleared(CaseKind::KnownHash);
    // The one eligible shape, and its boundary.
    assert_eq!(reaper_eligibility(&c, Some(end), end + 1, false), Ok(()));
    assert_eq!(
        reaper_eligibility(&c, Some(end), end, false),
        Err(Ineligible::BeforeEnd)
    );
    assert_eq!(
        reaper_eligibility(&c, Some(end), end - 1, false),
        Err(Ineligible::BeforeEnd)
    );
    assert_eq!(
        reaper_eligibility(&c, None, u64::MAX, false),
        Err(Ineligible::BeforeEnd)
    );
    assert_eq!(
        reaper_eligibility(&c, Some(end), end + 1, true),
        Err(Ineligible::SharedFile)
    );
    let held = set_evidence_legal_hold(&c, &two(), "hold", 1).unwrap();
    assert_eq!(
        reaper_eligibility(&held, Some(end), end + 1, false),
        Err(Ineligible::LegalHold)
    );
    // Undecided and confirmed are never eligible, however far past the end.
    assert_eq!(
        reaper_eligibility(&case(CaseKind::KnownHash), Some(end), u64::MAX, false),
        Err(Ineligible::Undecided)
    );
    let confirmed = confirm(&case(CaseKind::KnownHash), 1).unwrap();
    assert_eq!(
        reaper_eligibility(&confirmed, Some(end), u64::MAX, false),
        Err(Ineligible::Confirmed)
    );
    // Beginning destruction records it, and only once.
    let d = begin_destruction_expired(&c, Some(end), end + 1, false).unwrap();
    assert_eq!(d.destruction.as_ref().unwrap().by, None);
    assert_eq!(
        d.destruction.as_ref().unwrap().basis,
        DestructionBasis::ClearedAndExpired
    );
    assert_eq!(
        reaper_eligibility(&d, Some(end), end + 1, false),
        Err(Ineligible::AlreadyBegun)
    );
    assert_eq!(
        begin_destruction_expired(&c, Some(end), end, false).unwrap_err(),
        CaseError::NotEligible(Ineligible::BeforeEnd)
    );
}

// ---- releasing holds -------------------------------------------------------------------------------

#[test]
fn an_evidence_hold_is_never_released_before_the_end_even_by_two_people() {
    let c = cleared(CaseKind::KnownHash);
    let end = 10_000u64;
    let t = two();
    for now in [0u64, end - 1, end] {
        assert_eq!(
            check_release_hold(&c, HoldTarget::EvidenceKey, Some(end), now, true, &t),
            Err(ReleaseRefusal::BeforeEnd),
            "now {now}"
        );
    }
    assert_eq!(
        check_release_hold(&c, HoldTarget::EvidenceKey, None, u64::MAX, true, &t),
        Err(ReleaseRefusal::BeforeEnd)
    );
    assert_eq!(
        check_release_hold(&c, HoldTarget::EvidenceKey, Some(end), end + 1, true, &t),
        Ok(())
    );
    let held = set_evidence_legal_hold(&c, &t, "hold", 1).unwrap();
    assert_eq!(
        check_release_hold(&held, HoldTarget::EvidenceKey, Some(end), end + 1, true, &t),
        Err(ReleaseRefusal::LegalHold)
    );
}

#[test]
fn a_serving_hold_is_released_only_on_a_cleared_case_once_the_evidence_copy_is_held() {
    let t = two();
    let c = cleared(CaseKind::KnownHash);
    assert_eq!(
        check_release_hold(&c, HoldTarget::ServingKey, Some(1 << 60), 0, true, &t),
        Ok(()),
        "the serving hold may go at CLEAR, long before the end"
    );
    assert_eq!(
        check_release_hold(&c, HoldTarget::ServingKey, Some(1 << 60), 0, false, &t),
        Err(ReleaseRefusal::EvidenceCopyNotHeld)
    );
    // Undecided and confirmed: neither hold goes.
    for case in [
        case(CaseKind::KnownHash),
        confirm(&self::case(CaseKind::KnownHash), 1).unwrap(),
    ] {
        for target in [HoldTarget::ServingKey, HoldTarget::EvidenceKey] {
            assert_eq!(
                check_release_hold(&case, target, Some(10), u64::MAX, true, &t),
                Err(ReleaseRefusal::NotCleared)
            );
        }
    }
}

// ---- the crate cannot delete -------------------------------------------------------------------------

#[test]
fn nothing_in_the_crate_can_delete_anything() {
    for (name, src) in [
        ("evidence/case.rs", include_str!("../src/evidence/case.rs")),
        ("evidence/mod.rs", include_str!("../src/evidence/mod.rs")),
        (
            "evidence/preserve.rs",
            include_str!("../src/evidence/preserve.rs"),
        ),
        (
            "evidence/state.rs",
            include_str!("../src/evidence/state.rs"),
        ),
        (
            "evidence/people.rs",
            include_str!("../src/evidence/people.rs"),
        ),
        ("report/state.rs", include_str!("../src/report/state.rs")),
    ] {
        for line in src.lines().map(str::trim_start) {
            if line.starts_with("//") {
                continue;
            }
            assert!(
                !(line.contains("fn delete")
                    || line.contains("fn remove")
                    || line.contains("fn destroy")
                    || line.contains("fn purge")),
                "{name}: a delete-like function: {line}"
            );
            assert!(!line.contains("std::fs"), "{name}: file access: {line}");
        }
    }
}
