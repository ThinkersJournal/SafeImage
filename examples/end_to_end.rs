//! One image through every layer, with scripted stand-ins for the parts that live outside this
//! repository (a provider profile and a destination profile). Run it with:
//!
//! ```text
//! cargo run --example end_to_end
//! ```
//!
//! The stand-ins below speak a made-up wire format; a real profile implements the same traits for
//! a real service. Nothing here touches a network, a clock or a disk: the crate is sans-I/O, so the
//! "host" in this file is just `main` performing the exchanges by hand.

use safeimage::evidence::{
    ActorId, CaseKind, CaseRecord, ClearApproval, EvidenceEvent, EvidenceState, FileRecord,
    ReportClock, TwoPeople, apply_file_event, case_evidence, clear, preserve_until,
};
use safeimage::hash::{Frame, pdq_dihedral};
use safeimage::matcher::{
    CleanBasis, HttpMethod, HttpRequestSpec, HttpResponseSpec, MAX_RESPONSE_BYTES, MatchOutcome,
    MatchRequest, ProtocolError, ProviderProtocol, Unavailable, classify_status, within_cap,
};
use safeimage::report::{
    BuildError, BuiltReport, CredentialSlot, Credentials, DestinationAnswer, DestinationId, Person,
    ReportDestination, ReportFile, ReportInput, ReportState, Secret, SetupState, SlotKey, SlotKind,
    Status, Step, apply, next_step, setup_state,
};
use safeimage::screening::{ScreenPolicy, Screening, match_request, screen};

// ---- a stand-in provider profile: the "wire format" is the literal body text --------------------

struct ToyProvider;

impl ProviderProtocol for ToyProvider {
    fn request(&self, r: &MatchRequest) -> Result<HttpRequestSpec, ProtocolError> {
        Ok(HttpRequestSpec {
            method: HttpMethod::Post,
            path: "/lookup".into(),
            headers: vec![],
            body: r.hashes().iter().flat_map(|h| h.to_bytes()).collect(),
        })
    }

    fn interpret(&self, _r: &MatchRequest, resp: &HttpResponseSpec) -> MatchOutcome {
        if let Some(reason) = classify_status(resp.status) {
            return MatchOutcome::Unavailable(reason);
        }
        if !within_cap(&resp.body, MAX_RESPONSE_BYTES) {
            return MatchOutcome::Unavailable(Unavailable::MalformedResponse);
        }
        match resp.body.as_slice() {
            b"NO-MATCH" => MatchOutcome::Clean(CleanBasis::default()),
            // Anything else, including a match this toy does not model, fails closed.
            _ => MatchOutcome::Unavailable(Unavailable::MalformedResponse),
        }
    }
}

// ---- a stand-in destination profile ---------------------------------------------------------------

struct ToyDestination {
    id: DestinationId,
    slots: Vec<CredentialSlot>,
}

impl ReportDestination for ToyDestination {
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
        Ok(BuiltReport {
            bytes: format!("report of {} file(s)", input.files.len()).into_bytes(),
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
            path: format!(
                "/{}",
                format!("{step:?}").split(' ').next().unwrap_or("step")
            )
            .to_lowercase(),
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

fn main() {
    // 1. A decoded frame. In a real host the decoder produces this; here it is a gradient.
    let (w, h) = (256u32, 192u32);
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            rgb.extend_from_slice(&[(x ^ y) as u8, (x * 255 / w) as u8, (y * 255 / h) as u8]);
        }
    }
    let frame = Frame::new(&rgb, w, h).expect("dimensions match the buffer");

    // 2. Layer 1: hash it in all eight orientations.
    let d = pdq_dihedral(&frame);
    println!("quality {} ; first hash {}", d.quality, d.hashes[0]);

    // 3. Screening step one: the request to send (refuses an unhashable frame).
    let policy = ScreenPolicy::default();
    let request = match_request(&frame, &policy).expect("a patterned frame is hashable");
    println!("request carries {} hashes", request.hashes().len());

    // 4. Layer 2: the host performs the exchange; here the response is scripted.
    let provider = ToyProvider;
    let spec = provider.request(&request).expect("request builds");
    println!(
        "send {:?} {} ({} bytes)",
        spec.method,
        spec.path,
        spec.body.len()
    );
    let response = HttpResponseSpec {
        status: 200,
        body: b"NO-MATCH".to_vec(),
    };
    let outcome = provider.interpret(&request, &response);

    // 5. Layer 5: the screening decision. Only a clean answer allows.
    match screen(&frame, outcome, None, &policy) {
        Screening::Allow {
            frame: screened, ..
        } => {
            println!(
                "allowed: a model may now receive a {}x{} frame",
                screened.frame().width(),
                screened.frame().height()
            );
        }
        other => panic!("expected Allow, got {other:?}"),
    }

    // A failure of the provider is a hold, never an allow.
    let down = provider.interpret(
        &request,
        &HttpResponseSpec {
            status: 503,
            body: vec![],
        },
    );
    assert!(matches!(
        screen(&frame, down, None, &policy),
        Screening::Hold(_)
    ));
    println!("provider outage: held, not published");

    // 6. Layer 4: a report through a destination profile. Credentials stay with the host.
    let dest = ToyDestination {
        id: DestinationId::new("example-destination").unwrap(),
        slots: vec![CredentialSlot {
            key: SlotKey::new("token").unwrap(),
            kind: SlotKind::Secret,
            required: true,
            label: None,
        }],
    };
    let creds = Credentials::new().with(
        SlotKey::new("token").unwrap(),
        Secret::new("not-a-real-secret"),
    );
    assert_eq!(setup_state(&[&dest], &creds), SetupState::Ready);
    assert_eq!(
        setup_state(&[], &creds),
        SetupState::NoDestinationsConfigured
    );

    let input = ReportInput {
        reporting_person: Person {
            name: "Operator".into(),
            ..Person::default()
        },
        reported_person: Person::default(),
        incident_at: 1_000,
        files: vec![ReportFile {
            sha256_hex: "00".repeat(32),
            viewed_by_operator: false,
            publicly_available: true,
        }],
        pages: vec![],
    };
    let built = dest.build(&input).expect("report builds");
    let mut state = ReportState::new(input.files.len(), true);
    let mut now = 2_000u64;
    while state.status != Status::Finished {
        let step = next_step(&state, now);
        let req = dest.request(&step, &state, &built).expect("request builds");
        // The host's transport would send `req` (with the credential applied); here it succeeds.
        let answer = dest.interpret(
            &step,
            &HttpResponseSpec {
                status: 200,
                body: vec![],
            },
        );
        let (next, event) = apply(&state, &step, &answer, now);
        println!("{:?} -> {:?} {}", step, next.status, req.path);
        assert!(event.is_none());
        state = next;
        now += 1;
    }

    // 7. Layer 3: evidence and the preservation clock.
    let case = CaseRecord::new(
        CaseKind::KnownHash,
        1_000,
        vec![FileRecord {
            sha256_hex: "00".repeat(32),
            state: EvidenceState::Pending,
        }],
    );
    let case = apply_file_event(&case, 0, &EvidenceEvent::PutSucceeded).unwrap();
    println!(
        "evidence: {:?}",
        case_evidence(&case.files.iter().map(|f| f.state).collect::<Vec<_>>())
    );
    let people = TwoPeople::new(
        ActorId::new("admin-a").unwrap(),
        ActorId::new("admin-b").unwrap(),
    )
    .expect("two different people");
    let cleared = clear(&case, &ClearApproval::Two(people), 3_000).expect("two people may clear");
    let end = preserve_until(
        cleared.created_at,
        cleared.review.is_some(),
        &[ReportClock {
            status: state.status,
            sent_at: vec![2_000],
            finished_at: Some(2_005),
        }],
    );
    println!("evidence is kept until {end:?} (one period after the last send or finish)");
}
