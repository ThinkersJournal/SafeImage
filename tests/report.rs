//! Layer 4: the generic report mechanism, driven by a test-double destination that has no real
//! schema, endpoint or code.

use safeimage::matcher::{HttpMethod, HttpRequestSpec, HttpResponseSpec, ProtocolError};
use safeimage::report::*;

// ---- a test double -----------------------------------------------------------------------

struct Double {
    id: DestinationId,
    slots: Vec<CredentialSlot>,
}

impl Double {
    fn new(label: Option<&str>) -> Self {
        Double {
            id: DestinationId::new("dest-a").unwrap(),
            slots: vec![
                CredentialSlot {
                    key: SlotKey::new("user").unwrap(),
                    kind: SlotKind::Text,
                    required: true,
                    label: label.map(str::to_string),
                },
                CredentialSlot {
                    key: SlotKey::new("pass").unwrap(),
                    kind: SlotKind::Secret,
                    required: true,
                    label: None,
                },
                CredentialSlot {
                    key: SlotKey::new("note").unwrap(),
                    kind: SlotKind::Text,
                    required: false,
                    label: None,
                },
            ],
        }
    }
}

impl ReportDestination for Double {
    fn id(&self) -> &DestinationId {
        &self.id
    }

    fn credential_slots(&self) -> &[CredentialSlot] {
        &self.slots
    }

    fn build(&self, input: &ReportInput) -> Result<BuiltReport, BuildError> {
        if input.files.is_empty() {
            return Err(BuildError::NoFiles);
        }
        if input.reporting_person.name.trim().is_empty() {
            return Err(BuildError::MissingField("reporting_person.name"));
        }
        Ok(BuiltReport {
            bytes: format!("doc:{}", input.files.len()).into_bytes(),
            content_type: "text/plain".into(),
        })
    }

    fn request(
        &self,
        step: &Step,
        _report: &ReportState,
        built: &BuiltReport,
    ) -> Result<HttpRequestSpec, ProtocolError> {
        Ok(HttpRequestSpec {
            method: HttpMethod::Post,
            path: format!("/{step:?}"),
            headers: vec![],
            body: built.bytes.clone(),
        })
    }

    fn interpret(&self, _step: &Step, resp: &HttpResponseSpec) -> DestinationAnswer {
        match resp.status {
            200 => DestinationAnswer::Accepted {
                report_id: Some("R-1".into()),
            },
            401 | 403 => DestinationAnswer::AuthRefused,
            503 => DestinationAnswer::Transient,
            other => DestinationAnswer::Unknown(other.to_string()),
        }
    }
}

fn creds_full() -> Credentials {
    Credentials::new()
        .with(SlotKey::new("user").unwrap(), Secret::new("alice"))
        .with(SlotKey::new("pass").unwrap(), Secret::new("hunter2"))
}

fn accepted(id: Option<&str>) -> DestinationAnswer {
    DestinationAnswer::Accepted {
        report_id: id.map(str::to_string),
    }
}

/// Drives a report to the end with every step answered `Accepted`, returning the steps taken.
fn run_happy(files: usize) -> (ReportState, Vec<Step>) {
    let mut s = ReportState::new(files, true);
    let mut steps = vec![];
    for _ in 0..50 {
        let step = next_step(&s, 1000);
        if step == Step::Done {
            break;
        }
        let ans = if step == Step::Submit {
            accepted(Some("R-1"))
        } else {
            accepted(None)
        };
        let (n, ev) = apply(&s, &step, &ans, 1000);
        assert_eq!(ev, None, "{step:?}");
        steps.push(step);
        s = n;
    }
    (s, steps)
}

// ---- the drain --------------------------------------------------------------------------------

#[test]
fn happy_path_runs_submit_then_each_file_then_finish() {
    let (s, steps) = run_happy(2);
    assert_eq!(
        steps,
        vec![
            Step::Submit,
            Step::Upload { file: 0 },
            Step::Describe { file: 0 },
            Step::Upload { file: 1 },
            Step::Describe { file: 1 },
            Step::Finish,
        ]
    );
    assert_eq!(s.status, Status::Finished);
    assert_eq!(s.report_id.as_deref(), Some("R-1"));
    assert_eq!(next_step(&s, 2000), Step::Done);
}

#[test]
fn backoff_is_2_4_8_16_then_30_minutes() {
    let mins: Vec<u64> = (1..=8).map(|n| backoff_secs(n) / 60).collect();
    assert_eq!(mins, vec![2, 4, 8, 16, 30, 30, 30, 30]);
    assert_eq!(backoff_secs(0) / 60, 2);
    assert_eq!(backoff_secs(u32::MAX) / 60, 30);
}

#[test]
fn a_transient_answer_waits_with_backoff_and_changes_no_status() {
    let mut s = ReportState::new(1, true);
    for n in 1..=6u32 {
        let (n_s, ev) = apply(&s, &Step::Submit, &DestinationAnswer::Transient, 10_000);
        assert_eq!(ev, None);
        assert_eq!(n_s.status, Status::Pending);
        assert_eq!(n_s.attempts, n);
        assert_eq!(n_s.next_attempt_at, Some(10_000 + backoff_secs(n)));
        // Not due until the time passes.
        assert_eq!(
            next_step(&n_s, 10_000),
            Step::Wait {
                until: 10_000 + backoff_secs(n)
            }
        );
        assert_eq!(next_step(&n_s, 10_000 + backoff_secs(n)), Step::Submit);
        s = n_s;
    }
}

#[test]
fn unknown_codes_never_advance_any_step() {
    let steps_and_states = {
        let pending = ReportState::new(1, true);
        let (submitted, _) = apply(&pending, &Step::Submit, &accepted(Some("R")), 0);
        let (uploaded, _) = apply(&submitted, &Step::Upload { file: 0 }, &accepted(None), 0);
        let (described, _) = apply(&uploaded, &Step::Describe { file: 0 }, &accepted(None), 0);
        let retracting = request_retract(&submitted).unwrap();
        vec![
            (pending, Step::Submit),
            (submitted.clone(), Step::Upload { file: 0 }),
            (uploaded, Step::Describe { file: 0 }),
            (described, Step::Finish),
            (retracting, Step::Retract),
        ]
    };
    for (state, step) in steps_and_states {
        let (after, ev) = apply(&state, &step, &DestinationAnswer::Unknown("9999".into()), 0);
        assert_eq!(after.status, state.status, "{step:?}");
        assert_eq!(after.files, state.files, "{step:?}");
        assert_eq!(after.report_id, state.report_id, "{step:?}");
        assert_eq!(after.attempts, 1, "{step:?}");
        assert!(
            matches!(ev, Some(Event::UnknownAnswer(c)) if c == "9999"),
            "{step:?}"
        );
        assert_ne!(after.status, Status::Finished);
    }
}

#[test]
fn a_submit_accepted_without_an_id_does_not_become_submitted() {
    let s = ReportState::new(1, true);
    for ans in [accepted(None), accepted(Some("  "))] {
        let (after, ev) = apply(&s, &Step::Submit, &ans, 0);
        assert_eq!(after.status, Status::Pending);
        assert_eq!(after.report_id, None);
        assert_eq!(ev, Some(Event::MissingReportId));
    }
}

#[test]
fn auth_refusal_keeps_the_status_raises_an_event_and_backs_off() {
    let s = ReportState::new(1, true);
    let (after, ev) = apply(&s, &Step::Submit, &DestinationAnswer::AuthRefused, 500);
    assert_eq!(after.status, Status::Pending);
    assert_eq!(ev, Some(Event::AuthRefused));
    assert_eq!(after.next_attempt_at, Some(500 + backoff_secs(1)));
}

#[test]
fn invalid_is_terminal_until_a_human_retries() {
    let s = ReportState::new(2, true);
    let (failed, ev) = apply(
        &s,
        &Step::Submit,
        &DestinationAnswer::Invalid("x".repeat(1000)),
        0,
    );
    assert_eq!(failed.status, Status::Failed);
    assert!(matches!(ev, Some(Event::Invalid(d)) if d.len() == 256));
    assert_eq!(next_step(&failed, u64::MAX), Step::Hold(HoldReason::Failed));
    // The drain cannot move it.
    let (same, ev) = apply(&failed, &Step::Submit, &accepted(Some("R")), 0);
    assert_eq!(same, failed);
    assert_eq!(ev, Some(Event::StepNotValidHere));
    // A human can.
    let again = retry_failed(&failed);
    assert_eq!(again.status, Status::Pending);
    assert_eq!(next_step(&again, 0), Step::Submit);
    // And only a Failed report is touched.
    assert_eq!(retry_failed(&s), s);
}

#[test]
fn already_finished_on_finish_is_success_and_keeps_the_note() {
    let (submitted, _) = apply(
        &ReportState::new(0, true),
        &Step::Submit,
        &accepted(Some("R")),
        0,
    );
    let (done, ev) = apply(
        &submitted,
        &Step::Finish,
        &DestinationAnswer::AlreadyFinished,
        0,
    );
    assert_eq!(done.status, Status::Finished);
    assert_eq!(done.finish_note, Some(FinishNote::AlreadyFinished));
    assert_eq!(ev, None);
}

#[test]
fn already_finished_on_retract_is_finished_and_flagged_nothing_released() {
    let (submitted, _) = apply(
        &ReportState::new(0, true),
        &Step::Submit,
        &accepted(Some("R")),
        0,
    );
    let retracting = request_retract(&submitted).unwrap();
    assert_eq!(retracting.status, Status::RetractPending);
    let (done, ev) = apply(
        &retracting,
        &Step::Retract,
        &DestinationAnswer::AlreadyFinished,
        0,
    );
    assert_eq!(done.status, Status::Finished);
    assert_eq!(done.finish_note, Some(FinishNote::FinishedBeforeRetract));
    assert_eq!(ev, Some(Event::FinishedBeforeRetract));
}

#[test]
fn already_finished_on_a_non_finish_step_is_not_success() {
    let (submitted, _) = apply(
        &ReportState::new(1, true),
        &Step::Submit,
        &accepted(Some("R")),
        0,
    );
    let (after, ev) = apply(
        &submitted,
        &Step::Upload { file: 0 },
        &DestinationAnswer::AlreadyFinished,
        0,
    );
    assert_eq!(after.status, Status::Submitted);
    assert!(!after.files[0].uploaded);
    assert_eq!(ev, Some(Event::StepNotValidHere));
}

#[test]
fn window_expiry_abandons_the_id_clears_progress_and_goes_back_to_pending() {
    let mut s = ReportState::new(2, true);
    for (step, ans) in [
        (Step::Submit, accepted(Some("R-old"))),
        (Step::Upload { file: 0 }, accepted(None)),
        (Step::Describe { file: 0 }, accepted(None)),
    ] {
        s = apply(&s, &step, &ans, 0).0;
    }
    assert!(s.files[0].uploaded && s.files[0].described);
    let (after, ev) = apply(
        &s,
        &Step::Upload { file: 1 },
        &DestinationAnswer::WindowExpired,
        0,
    );
    assert_eq!(ev, Some(Event::WindowExpired));
    assert_eq!(after.status, Status::Pending);
    assert_eq!(after.report_id, None);
    assert_eq!(after.abandoned_ids, vec!["R-old".to_string()]);
    assert!(after.files.iter().all(|f| *f == FileProgress::default()));
    assert_eq!(next_step(&after, 0), Step::Submit);
}

#[test]
fn window_expiry_during_a_withdrawal_ends_it() {
    let (submitted, _) = apply(
        &ReportState::new(1, true),
        &Step::Submit,
        &accepted(Some("R")),
        0,
    );
    let r = request_retract(&submitted).unwrap();
    let (after, _) = apply(&r, &Step::Retract, &DestinationAnswer::WindowExpired, 0);
    assert_eq!(after.status, Status::Withdrawn);
    assert_eq!(after.abandoned_ids, vec!["R".to_string()]);
}

#[test]
fn withdrawing_before_any_send_needs_no_destination_call() {
    for ready in [true, false] {
        let s = ReportState::new(1, ready);
        let w = request_retract(&s).unwrap();
        assert_eq!(w.status, Status::Withdrawn);
        assert_eq!(next_step(&w, 0), Step::Done);
    }
    let (done, _) = {
        let (sub, _) = apply(
            &ReportState::new(0, true),
            &Step::Submit,
            &accepted(Some("R")),
            0,
        );
        apply(&sub, &Step::Finish, &accepted(None), 0)
    };
    assert_eq!(request_retract(&done), Err(RetractError::AlreadyFinished));
}

#[test]
fn a_step_in_the_wrong_status_changes_nothing() {
    let s = ReportState::new(1, true);
    for step in [
        Step::Finish,
        Step::Retract,
        Step::Upload { file: 0 },
        Step::Describe { file: 0 },
        Step::Done,
        Step::Hold(HoldReason::Failed),
        Step::Wait { until: 5 },
    ] {
        let (after, ev) = apply(&s, &step, &accepted(None), 0);
        assert_eq!(after, s, "{step:?}");
        assert_eq!(ev, Some(Event::StepNotValidHere), "{step:?}");
    }
    // File index out of range.
    let (sub, _) = apply(&s, &Step::Submit, &accepted(Some("R")), 0);
    let (after, ev) = apply(&sub, &Step::Upload { file: 7 }, &accepted(None), 0);
    assert_eq!(after, sub);
    assert_eq!(ev, Some(Event::StepNotValidHere));
}

#[test]
fn deletion_window_is_past_the_later_of_the_two_deadlines() {
    let p = WindowPolicy {
        open_secs: 24 * 3600,
        idle_secs: 3600,
    };
    // opened 0, modified 90_000: later deadline is 90_000 + 3_600.
    assert!(!deletion_window_elapsed(0, 90_000, 93_600, p));
    assert!(deletion_window_elapsed(0, 90_000, 93_601, p));
    // opened 0, modified 0: later deadline is open + 24 h.
    assert!(!deletion_window_elapsed(0, 0, 86_400, p));
    assert!(deletion_window_elapsed(0, 0, 86_401, p));
}

// ---- credentials, setup and redaction -----------------------------------------------------------

#[test]
fn missing_required_credentials_hold_the_report_until_supplied() {
    let d = Double::new(None);
    let none = Credentials::new();
    assert!(!credentials_ready(&d, &none));
    let s = ReportState::new(1, credentials_ready(&d, &none));
    assert_eq!(s.status, Status::AwaitingCredentials);
    assert_eq!(
        next_step(&s, 0),
        Step::Hold(HoldReason::AwaitingCredentials)
    );

    // One of two required: still not ready. A blank value does not count.
    let partial = Credentials::new()
        .with(SlotKey::new("user").unwrap(), Secret::new("alice"))
        .with(SlotKey::new("pass").unwrap(), Secret::new("   "));
    assert!(!credentials_ready(&d, &partial));

    let ready = creds_full();
    assert!(credentials_ready(&d, &ready));
    let s = sync_credentials(&s, true);
    assert_eq!(s.status, Status::Pending);
    // And back, but a submitted report is never demoted.
    assert_eq!(
        sync_credentials(&s, false).status,
        Status::AwaitingCredentials
    );
    let (sub, _) = apply(&s, &Step::Submit, &accepted(Some("R")), 0);
    assert_eq!(sync_credentials(&sub, false).status, Status::Submitted);
}

#[test]
fn no_destinations_is_a_distinct_state_from_ready() {
    let c = creds_full();
    assert_eq!(setup_state(&[], &c), SetupState::NoDestinationsConfigured);
    let d = Double::new(None);
    assert_eq!(setup_state(&[&d], &c), SetupState::Ready);
    assert_ne!(setup_state(&[], &c), SetupState::Ready);
}

#[test]
fn prompts_are_generic_unless_the_profile_supplies_a_label() {
    let generic = Double::new(None);
    let SetupState::Prompts(p) = setup_state(&[&generic], &Credentials::new()) else {
        panic!("expected prompts");
    };
    assert_eq!(p.len(), 2); // the optional slot is not asked for
    assert_eq!(
        p[0].text,
        "Credentials for reporting destination `dest-a`, slot `user` (your registration documents say what to enter)"
    );
    let labelled = Double::new(Some("Account username"));
    let SetupState::Prompts(p) = setup_state(&[&labelled], &Credentials::new()) else {
        panic!("expected prompts");
    };
    assert_eq!(
        p[0].text,
        "Credentials for reporting destination `dest-a`: Account username"
    );
}

#[test]
fn identifiers_are_validated() {
    assert_eq!(DestinationId::new("").unwrap_err(), IdError::Empty);
    assert_eq!(
        DestinationId::new("a".repeat(65)).unwrap_err(),
        IdError::TooLong
    );
    assert_eq!(
        DestinationId::new("Has Space").unwrap_err(),
        IdError::BadCharacter
    );
    assert!(DestinationId::new("ok_name-1").is_ok());
    assert_eq!(SlotKey::new("UPPER").unwrap_err(), IdError::BadCharacter);
}

#[test]
fn secrets_and_personal_data_never_reach_debug_output() {
    let creds = creds_full();
    let person = Person {
        id: "u-1".into(),
        name: "Casey Example".into(),
        email: Some("casey@example.test".into()),
        phone: Some("+1-555-0100".into()),
        profile_url: Some("https://example.test/u/casey".into()),
        address: Some(Address {
            street: "1 Example Way".into(),
            city: "Exampleton".into(),
            region: "EX".into(),
            postal_code: "00000".into(),
            country: "XX".into(),
        }),
    };
    let input = ReportInput {
        reporting_person: person.clone(),
        reported_person: person,
        incident_at: 1,
        files: vec![ReportFile {
            sha256_hex: "ab".repeat(32),
            viewed_by_operator: false,
            publicly_available: true,
        }],
        pages: vec!["https://example.test/p/1".into()],
    };
    let built = Double::new(None).build(&input).unwrap();
    let rendered = [
        format!("{creds:?}"),
        format!("{creds:#?}"),
        format!("{:?}", Secret::new("hunter2")),
        format!("{input:?}"),
        format!("{input:#?}"),
        format!("{built:?}"),
        format!("{:?}", input.reporting_person),
        format!("{:?}", input.reporting_person.address),
        format!(
            "{:?}",
            Double::new(None).build(&ReportInput {
                files: vec![],
                ..input.clone()
            })
        ),
    ]
    .join("\n");
    for needle in [
        "hunter2",
        "alice",
        "Casey",
        "casey@example.test",
        "555-0100",
        "1 Example Way",
        "Exampleton",
        "doc:1",
    ] {
        assert!(!rendered.contains(needle), "leaked {needle:?}");
    }
    // Control: the slot keys, which are not secret, are visible, so the check can see strings.
    assert!(rendered.contains("user") && rendered.contains("pass"));
    assert!(rendered.contains("redacted"));
}

#[test]
fn a_request_spec_built_from_a_report_carries_no_credential() {
    let d = Double::new(None);
    let input = ReportInput {
        reporting_person: Person {
            name: "n".into(),
            ..Person::default()
        },
        reported_person: Person::default(),
        incident_at: 0,
        files: vec![ReportFile {
            sha256_hex: "00".repeat(32),
            viewed_by_operator: true,
            publicly_available: false,
        }],
        pages: vec![],
    };
    let built = d.build(&input).unwrap();
    let spec = d
        .request(&Step::Submit, &ReportState::new(1, true), &built)
        .unwrap();
    let text = format!("{spec:?}");
    assert!(!text.contains("hunter2") && !text.contains("alice"));
    assert!(spec.headers.is_empty());
}

#[test]
fn an_incomplete_address_is_omitted_not_sent_half_filled() {
    let full = Address {
        street: "s".into(),
        city: "c".into(),
        region: "r".into(),
        postal_code: "p".into(),
        country: "x".into(),
    };
    let mut p = Person {
        address: Some(full.clone()),
        ..Person::default()
    };
    assert!(p.reportable_address().is_some());
    for blank in 0..5 {
        let mut a = full.clone();
        match blank {
            0 => a.street = " ".into(),
            1 => a.city = String::new(),
            2 => a.region = String::new(),
            3 => a.postal_code = String::new(),
            _ => a.country = "\t".into(),
        }
        p.address = Some(a);
        assert!(p.reportable_address().is_none(), "blank {blank}");
    }
    p.address = None;
    assert!(p.reportable_address().is_none());
}

#[test]
fn the_double_reports_build_errors_by_field_name_only() {
    let d = Double::new(None);
    let mut input = ReportInput {
        reporting_person: Person::default(),
        reported_person: Person::default(),
        incident_at: 0,
        files: vec![],
        pages: vec![],
    };
    assert_eq!(d.build(&input).unwrap_err(), BuildError::NoFiles);
    input.files.push(ReportFile {
        sha256_hex: String::new(),
        viewed_by_operator: false,
        publicly_available: false,
    });
    assert_eq!(
        d.build(&input).unwrap_err(),
        BuildError::MissingField("reporting_person.name")
    );
}
