use crate::evidence::*;
use crate::evidence_codec::*;
use crate::*;
use safeimage::evidence::{
    self as ev, ActorId, CaseKind, CaseRecord, ClearApproval, EvidenceEvent, EvidenceState,
    FileRecord, ReportClock, TwoPeople,
};
use safeimage::report::Status as ReportStatus;

const BIG: usize = 1 << 16;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn actor(s: &str) -> ActorId {
    ActorId::new(s).unwrap()
}

fn two(a: &str, b: &str) -> TwoPeople {
    TwoPeople::new(actor(a), actor(b)).unwrap()
}

fn file(sha: &str, state: EvidenceState) -> FileRecord {
    FileRecord {
        sha256_hex: sha.into(),
        state,
    }
}

fn case(kind: CaseKind) -> CaseRecord {
    CaseRecord::new(
        kind,
        1_000,
        vec![
            file("aa", EvidenceState::Present),
            file("bb", EvidenceState::Failed),
        ],
    )
}

fn cleared() -> CaseRecord {
    ev::clear(
        &case(CaseKind::Classifier),
        &ClearApproval::One(actor("m1")),
        2_000,
    )
    .unwrap()
}

fn ok_case(r: Result<Vec<u8>, i32>) -> CaseRecord {
    decode_case(&r.unwrap()).unwrap()
}

#[test]
fn case_round_trips_with_every_optional_part() {
    let mut c = cleared();
    c.evidence_hold = Some(ev::LegalHold {
        set_by: two("a", "b"),
        reason: "subpoena".into(),
        set_at: 5,
    });
    assert_eq!(decode_case(&encode_case(&c)).unwrap(), c);
    let d = ev::begin_destruction_on_request(&c, "LE-1", &two("a", "b"), 9).unwrap();
    assert_eq!(decode_case(&encode_case(&d)).unwrap(), d);
    let e = ev::begin_destruction_expired(&cleared(), Some(10), 11, false).unwrap();
    assert_eq!(decode_case(&encode_case(&e)).unwrap(), e);
}

#[test]
fn transition_matches_the_table_for_every_pair() {
    let states = [
        EvidenceState::Pending,
        EvidenceState::Present,
        EvidenceState::Failed,
        EvidenceState::Missing,
        EvidenceState::FileWithoutEvidence,
    ];
    let events = [
        EvidenceEvent::PutSucceeded,
        EvidenceEvent::PutFailed,
        EvidenceEvent::ObjectMissing,
        EvidenceEvent::DecidedWithoutEvidence(two("a", "b")),
    ];
    for s in states {
        for e in &events {
            let got = transition_code(u32::from(state_tag(s)), &encode_event(e));
            match ev::transition(s, e) {
                Ok(n) => assert_eq!(got, Ok(i32::from(state_tag(n)))),
                Err(_) => assert_eq!(got, Err(ERR_TRANSITION_REFUSED)),
            }
        }
    }
}

#[test]
fn derived_answers_match() {
    let all = [
        EvidenceState::Pending,
        EvidenceState::Present,
        EvidenceState::Failed,
        EvidenceState::Missing,
        EvidenceState::FileWithoutEvidence,
    ];
    for s in all {
        for found in [false, true] {
            let want = match ev::report_file_disposition(s, found) {
                ev::FileDisposition::SendWithFile => 0,
                ev::FileDisposition::SendWithoutFile => 1,
                ev::FileDisposition::Hold => 2,
            };
            assert_eq!(
                disposition_code(u32::from(state_tag(s)), u32::from(found)),
                Ok(want)
            );
        }
    }
    let list = |v: &[EvidenceState]| {
        let mut w = crate::codec::Writer::new();
        w.u32(v.len() as u32);
        for s in v {
            w.u8(state_tag(*s));
        }
        w.finish()
    };
    assert_eq!(case_evidence_code(&list(&[])), Ok(2));
    assert_eq!(case_evidence_code(&list(&[EvidenceState::Pending])), Ok(1));
    assert_eq!(
        case_evidence_code(&list(&[EvidenceState::Pending, EvidenceState::Missing])),
        Ok(0)
    );
    // An empty report may not submit; a held file blocks it.
    let pairs = |v: &[(EvidenceState, bool)]| {
        let mut w = crate::codec::Writer::new();
        w.u32(v.len() as u32);
        for (s, f) in v {
            w.u8(state_tag(*s));
            w.u8(u8::from(*f));
        }
        w.finish()
    };
    assert_eq!(may_submit_code(&pairs(&[])), Ok(0));
    assert_eq!(
        may_submit_code(&pairs(&[(EvidenceState::Present, true)])),
        Ok(1)
    );
    assert_eq!(
        may_submit_code(&pairs(&[
            (EvidenceState::Present, true),
            (EvidenceState::Present, false)
        ])),
        Ok(0)
    );
}

#[test]
fn preservation_clock_matches() {
    assert_eq!(si_preservation_period_secs(), 366 * 24 * 60 * 60);
    let done = ReportClock {
        status: ReportStatus::Finished,
        sent_at: vec![50, 90],
        finished_at: Some(100),
    };
    let open = ReportClock {
        status: ReportStatus::Pending,
        sent_at: vec![],
        finished_at: None,
    };
    for (decided, clocks) in [
        (true, vec![done.clone()]),
        (false, vec![done.clone()]),
        (true, vec![done.clone(), open.clone()]),
        (true, vec![]),
    ] {
        let want = ev::preserve_until(10, decided, &clocks);
        let got = preserve_until_bytes(10, u32::from(decided), &encode_clocks(&clocks)).unwrap();
        match want {
            None => assert_eq!(got[0], 0),
            Some(t) => {
                assert_eq!(got[0], 1);
                assert_eq!(u64::from_le_bytes(got[1..].try_into().unwrap()), t);
            }
        }
    }
    // Undecided means keep everything.
    assert_eq!(
        preserve_until_bytes(10, 0, &encode_clocks(&[done])).unwrap()[0],
        0
    );
}

#[test]
fn case_operations_match_the_rust_api() {
    let c = case(CaseKind::KnownHash);
    let bytes = encode_case(&c);

    // File events: a missing file, a refused transition and a valid one.
    let put = encode_event(&EvidenceEvent::PutSucceeded);
    assert_eq!(
        ok_case(apply_file_event_bytes(&bytes, 1, &put, BIG)),
        ev::apply_file_event(&c, 1, &EvidenceEvent::PutSucceeded).unwrap()
    );
    assert_eq!(
        apply_file_event_bytes(&bytes, 9, &put, BIG),
        Err(CASE_NO_SUCH_FILE)
    );
    assert_eq!(
        apply_file_event_bytes(&bytes, 0, &put, BIG),
        Err(ERR_TRANSITION_REFUSED)
    );

    // Confirm, then a second decision is refused.
    let confirmed = ok_case(confirm_bytes(&bytes, 7, BIG));
    assert_eq!(confirmed, ev::confirm(&c, 7).unwrap());
    assert_eq!(
        confirm_bytes(&encode_case(&confirmed), 8, BIG),
        Err(CASE_ALREADY_DECIDED)
    );

    // A known-hash case needs two people to clear; one is refused.
    let one = encode_approval(&ClearApproval::One(actor("m1")));
    assert_eq!(
        clear_bytes(&bytes, &one, 1, BIG),
        Err(CASE_NEEDS_TWO_PEOPLE)
    );
    let twop = encode_approval(&ClearApproval::Two(two("m1", "m2")));
    assert_eq!(
        ok_case(clear_bytes(&bytes, &twop, 1, BIG)).outcome(),
        Some(ev::ReviewOutcome::FalsePositive)
    );
    let sighting = encode_case(&case(CaseKind::ModeratorSighting));
    assert_eq!(
        clear_bytes(&sighting, &twop, 1, BIG),
        Err(CASE_NOT_CLEARABLE)
    );
}

#[test]
fn two_person_rule_is_checked_on_decode() {
    // Equal ids cannot be built into a pair, so a hold by "the same person twice" is refused.
    let mut w = crate::codec::Writer::new();
    w.str("same");
    w.str("same");
    let same = w.finish();
    let bytes = encode_case(&case(CaseKind::KnownHash));
    assert_eq!(
        set_hold_bytes(&bytes, &same, b"reason", 1, BIG),
        Err(ERR_SAME_PERSON)
    );
    assert_eq!(lift_hold_bytes(&bytes, &same, BIG), Err(ERR_SAME_PERSON));
    // A blank actor id is malformed.
    let mut w = crate::codec::Writer::new();
    w.str(" ");
    w.str("b");
    assert_eq!(
        set_hold_bytes(&bytes, &w.finish(), b"reason", 1, BIG),
        Err(ERR_MALFORMED)
    );
    // The same rule in an event.
    let mut w = crate::codec::Writer::new();
    w.u8(3);
    w.str("x");
    w.str("x");
    assert_eq!(transition_code(2, &w.finish()), Err(ERR_SAME_PERSON));
}

#[test]
fn holds_and_destruction_are_recorded_never_performed() {
    let c = cleared();
    let bytes = encode_case(&c);
    let by = encode_two(&two("a", "b"));
    assert_eq!(set_hold_bytes(&bytes, &by, b"  ", 1, BIG), Err(CASE_BLANK));
    let held = set_hold_bytes(&bytes, &by, b"subpoena", 1, BIG).unwrap();
    assert_eq!(
        set_hold_bytes(&held, &by, b"again", 2, BIG),
        Err(CASE_HOLD_ALREADY_SET)
    );
    assert_eq!(lift_hold_bytes(&bytes, &by, BIG), Err(CASE_NO_HOLD));
    assert!(
        decode_case(&lift_hold_bytes(&held, &by, BIG).unwrap())
            .unwrap()
            .evidence_hold
            .is_none()
    );

    // The reaper: before the end, held, shared and eligible.
    assert_eq!(
        eligibility_code(&bytes, 1, 100, 50, 0),
        Err(INELIGIBLE_BEFORE_END)
    );
    assert_eq!(
        eligibility_code(&bytes, 0, 0, 500, 0),
        Err(INELIGIBLE_BEFORE_END)
    );
    assert_eq!(
        eligibility_code(&held, 1, 100, 500, 0),
        Err(INELIGIBLE_LEGAL_HOLD)
    );
    assert_eq!(
        eligibility_code(&bytes, 1, 100, 500, 1),
        Err(INELIGIBLE_SHARED_FILE)
    );
    assert_eq!(eligibility_code(&bytes, 1, 100, 500, 0), Ok(0));
    let undecided = encode_case(&case(CaseKind::Classifier));
    assert_eq!(
        eligibility_code(&undecided, 1, 100, 500, 0),
        Err(INELIGIBLE_UNDECIDED)
    );

    // Begin destruction: recorded, then the case refuses every other write.
    let destroyed = destroy_expired_bytes(&bytes, 1, 100, 500, 0, BIG).unwrap();
    assert!(decode_case(&destroyed).unwrap().destruction.is_some());
    assert_eq!(confirm_bytes(&destroyed, 1, BIG), Err(CASE_DESTROYED));
    assert_eq!(
        set_hold_bytes(&destroyed, &by, b"x", 1, BIG),
        Err(CASE_DESTROYED)
    );
    assert_eq!(
        destroy_on_request_bytes(&destroyed, b"LE", &by, 1, BIG),
        Err(CASE_DESTRUCTION_ALREADY_BEGUN)
    );
    assert_eq!(
        destroy_on_request_bytes(&bytes, b" ", &by, 1, BIG),
        Err(CASE_BLANK)
    );
    let le = destroy_on_request_bytes(&held, b"LE-42", &by, 3, BIG).unwrap();
    assert!(decode_case(&le).unwrap().destruction.is_some());
}

#[test]
fn release_hold_rules() {
    let c = cleared();
    let bytes = encode_case(&c);
    let by = encode_two(&two("a", "b"));
    // Serving key: needs the evidence copies held.
    assert_eq!(release_code_for(&bytes, 0, 0, 0, 0, 1, &by), Ok(0));
    assert_eq!(
        release_code_for(&bytes, 0, 0, 0, 0, 0, &by),
        Err(RELEASE_EVIDENCE_COPY_NOT_HELD)
    );
    // Evidence key: never before the end, even by two people.
    assert_eq!(
        release_code_for(&bytes, 1, 1, 100, 100, 1, &by),
        Err(RELEASE_BEFORE_END)
    );
    assert_eq!(release_code_for(&bytes, 1, 1, 100, 101, 1, &by), Ok(0));
    assert_eq!(
        release_code_for(&bytes, 2, 1, 100, 101, 1, &by),
        Err(ERR_BAD_ARGUMENT)
    );
    let confirmed = encode_case(&ev::confirm(&case(CaseKind::KnownHash), 1).unwrap());
    assert_eq!(
        release_code_for(&confirmed, 0, 0, 0, 0, 1, &by),
        Err(RELEASE_NOT_CLEARED)
    );
}

#[test]
fn unknown_tags_and_truncation_are_refusals() {
    let mut c = cleared();
    c.evidence_hold = Some(ev::LegalHold {
        set_by: two("a", "b"),
        reason: "r".into(),
        set_at: 1,
    });
    let full = encode_case(&c);
    for n in 0..full.len() {
        assert!(
            confirm_bytes(&full[..n], 1, BIG).is_err(),
            "case prefix {n}"
        );
    }
    let mut bad = full.clone();
    bad[1] = 9; // case kind
    assert_eq!(confirm_bytes(&bad, 1, BIG), Err(ERR_UNKNOWN_TAG));
    assert_eq!(
        transition_code(9, &encode_event(&EvidenceEvent::PutSucceeded)),
        Err(ERR_UNKNOWN_TAG)
    );
    assert_eq!(
        transition_code(1, &[crate::codec::VERSION, 9]),
        Err(ERR_UNKNOWN_TAG)
    );
    assert_eq!(
        transition_code(300, &encode_event(&EvidenceEvent::PutSucceeded)),
        Err(ERR_BAD_ARGUMENT)
    );
    // A hostile file count with nothing behind it: truncated, never allocated.
    let mut hostile = vec![crate::codec::VERSION, 0];
    hostile.extend_from_slice(&0u64.to_le_bytes());
    hostile.push(0); // review none
    hostile.extend_from_slice(&[255, 255, 255, 255]);
    assert_eq!(confirm_bytes(&hostile, 1, BIG), Err(ERR_TRUNCATED));
    assert_eq!(confirm_bytes(&full, 1, 1), Err(CASE_ALREADY_DECIDED));
    let undecided = encode_case(&case(CaseKind::KnownHash));
    assert_eq!(confirm_bytes(&undecided, 1, 1), Err(ERR_OUT_TOO_SMALL));
}

/// Vectors for a TypeScript host; regenerate with `SAFEIMAGE_WRITE_VECTORS=1`.
#[test]
fn layer3_vectors_file_is_current() {
    fn res(r: Result<Vec<u8>, i32>) -> String {
        match r {
            Ok(b) => hex(&b),
            Err(c) => c.to_string(),
        }
    }
    fn num(r: Result<i32, i32>) -> String {
        match r {
            Ok(v) => v.to_string(),
            Err(c) => c.to_string(),
        }
    }
    let mut t =
        String::from("# layer 3 vectors: <kind> <name> <args...> <result: hex or decimal code>\n");
    let states = [
        EvidenceState::Pending,
        EvidenceState::Present,
        EvidenceState::Failed,
        EvidenceState::Missing,
        EvidenceState::FileWithoutEvidence,
    ];
    let events = [
        ("put_ok", EvidenceEvent::PutSucceeded),
        ("put_fail", EvidenceEvent::PutFailed),
        ("missing", EvidenceEvent::ObjectMissing),
        (
            "decided",
            EvidenceEvent::DecidedWithoutEvidence(two("alice", "bob")),
        ),
    ];
    for s in states {
        for (n, e) in &events {
            let st = u32::from(state_tag(s));
            t += &format!(
                "transition {st}_{n} {st} {} {}\n",
                hex(&encode_event(e)),
                num(transition_code(st, &encode_event(e)))
            );
        }
        for found in [0u32, 1] {
            let st = u32::from(state_tag(s));
            t += &format!(
                "disposition {st}_{found} {st} {found} {}\n",
                num(disposition_code(st, found))
            );
        }
    }
    t += &format!("period {}\n", si_preservation_period_secs());

    let done = ReportClock {
        status: ReportStatus::Finished,
        sent_at: vec![50, 90],
        finished_at: Some(100),
    };
    let open = ReportClock {
        status: ReportStatus::Pending,
        sent_at: vec![],
        finished_at: None,
    };
    for (name, decided, clocks) in [
        ("finished", 1u32, vec![done.clone()]),
        ("undecided", 0, vec![done.clone()]),
        ("open", 1, vec![done, open]),
    ] {
        let b = encode_clocks(&clocks);
        t += &format!(
            "preserve {name} 10 {decided} {} {}\n",
            hex(&b),
            hex(&preserve_until_bytes(10, decided, &b).unwrap())
        );
    }

    let known = case(CaseKind::KnownHash);
    let known_b = encode_case(&known);
    let by = encode_two(&two("alice", "bob"));
    let put = encode_event(&EvidenceEvent::PutSucceeded);
    t += &format!(
        "file_event ok {} 1 {} {}\n",
        hex(&known_b),
        hex(&put),
        res(apply_file_event_bytes(&known_b, 1, &put, BIG))
    );
    t += &format!(
        "file_event bad_index {} 9 {} {}\n",
        hex(&known_b),
        hex(&put),
        res(apply_file_event_bytes(&known_b, 9, &put, BIG))
    );
    t += &format!(
        "confirm ok {} 7 {}\n",
        hex(&known_b),
        res(confirm_bytes(&known_b, 7, BIG))
    );
    let one = encode_approval(&ClearApproval::One(actor("m1")));
    let twop = encode_approval(&ClearApproval::Two(two("m1", "m2")));
    t += &format!(
        "clear one_refused {} {} 5 {}\n",
        hex(&known_b),
        hex(&one),
        res(clear_bytes(&known_b, &one, 5, BIG))
    );
    t += &format!(
        "clear two_ok {} {} 5 {}\n",
        hex(&known_b),
        hex(&twop),
        res(clear_bytes(&known_b, &twop, 5, BIG))
    );
    let cl = encode_case(&cleared());
    t += &format!(
        "set_hold ok {} {} {} 9 {}\n",
        hex(&cl),
        hex(&by),
        hex(b"subpoena"),
        res(set_hold_bytes(&cl, &by, b"subpoena", 9, BIG))
    );
    let held = set_hold_bytes(&cl, &by, b"subpoena", 9, BIG).unwrap();
    t += &format!(
        "lift ok {} {} {}\n",
        hex(&held),
        hex(&by),
        res(lift_hold_bytes(&held, &by, BIG))
    );
    t += &format!(
        "destroy_req ok {} {} {} 3 {}\n",
        hex(&held),
        hex(b"LE-42"),
        hex(&by),
        res(destroy_on_request_bytes(&held, b"LE-42", &by, 3, BIG))
    );
    t += &format!(
        "destroy_exp ok {} 1 100 500 0 {}\n",
        hex(&cl),
        res(destroy_expired_bytes(&cl, 1, 100, 500, 0, BIG))
    );
    t += &format!(
        "destroy_exp early {} 1 100 50 0 {}\n",
        hex(&cl),
        res(destroy_expired_bytes(&cl, 1, 100, 50, 0, BIG))
    );
    t += &format!(
        "eligible held {} 1 100 500 0 {}\n",
        hex(&held),
        num(eligibility_code(&held, 1, 100, 500, 0))
    );
    t += &format!(
        "eligible ok {} 1 100 500 0 {}\n",
        hex(&cl),
        num(eligibility_code(&cl, 1, 100, 500, 0))
    );
    t += &format!(
        "release serving {} 0 0 0 0 1 {} {}\n",
        hex(&cl),
        hex(&by),
        num(release_code_for(&cl, 0, 0, 0, 0, 1, &by))
    );
    t += &format!(
        "release evidence_early {} 1 1 100 100 1 {} {}\n",
        hex(&cl),
        hex(&by),
        num(release_code_for(&cl, 1, 1, 100, 100, 1, &by))
    );
    t += &format!(
        "release evidence_late {} 1 1 100 101 1 {} {}\n",
        hex(&cl),
        hex(&by),
        num(release_code_for(&cl, 1, 1, 100, 101, 1, &by))
    );

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/layer3.txt");
    if std::env::var_os("SAFEIMAGE_WRITE_VECTORS").is_some() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors")).unwrap();
        std::fs::write(path, &t).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), t);
}
