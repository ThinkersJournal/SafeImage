//! Layer 2: outcome ordering, the mock, the retry rule and the conformance suite.

use safeimage::hash::Hash256;
use safeimage::matcher::*;

fn m(c: Classification, t: MatchType) -> MatchOutcome {
    MatchOutcome::Match(KnownMatch::new(c, t, vec![]))
}

fn clean() -> MatchOutcome {
    MatchOutcome::Clean(CleanBasis::default())
}

fn req() -> MatchRequest {
    MatchRequest::pdq(vec![Hash256::zero()]).unwrap()
}

const ALL_REASONS: [Unavailable; 8] = [
    Unavailable::NotConfigured,
    Unavailable::Disabled,
    Unavailable::Timeout,
    Unavailable::ServerError,
    Unavailable::Throttled,
    Unavailable::CredentialsRefused,
    Unavailable::MalformedResponse,
    Unavailable::TestValueInProduction,
];

// ---- I1 / I2: ordering ----------------------------------------------------------------------

#[test]
fn a_failure_is_never_weaker_than_a_clean_answer() {
    for r in ALL_REASONS {
        assert!(MatchOutcome::Unavailable(r) > clean(), "{r:?}");
        assert!(!MatchOutcome::Unavailable(r).is_clean(), "{r:?}");
        assert_eq!(
            strongest([clean(), MatchOutcome::Unavailable(r), clean()]),
            MatchOutcome::Unavailable(r)
        );
    }
}

#[test]
fn a_match_outranks_every_failure_and_every_clean_answer() {
    let hit = m(Classification::Other("x".into()), MatchType::Near);
    for r in ALL_REASONS {
        assert_eq!(
            strongest([MatchOutcome::Unavailable(r), hit.clone(), clean()]),
            hit
        );
    }
}

#[test]
fn only_clean_is_clean() {
    assert!(clean().is_clean());
    assert!(!m(Classification::Csam, MatchType::Exact).is_clean());
}

#[test]
fn strongest_does_not_depend_on_input_order() {
    let pool = vec![
        clean(),
        MatchOutcome::Unavailable(Unavailable::Timeout),
        MatchOutcome::Unavailable(Unavailable::CredentialsRefused),
        m(Classification::HarmfulAbusive, MatchType::Exact),
        m(Classification::Csam, MatchType::Near),
        m(Classification::Csam, MatchType::Exact),
        m(Classification::Other("z".into()), MatchType::Exact),
    ];
    // Every ordered pair and every rotation of the whole pool.
    for a in &pool {
        for b in &pool {
            assert_eq!(
                strongest([a.clone(), b.clone()]),
                strongest([b.clone(), a.clone()])
            );
        }
    }
    let expect = strongest(pool.clone());
    for k in 0..pool.len() {
        let mut rot = pool.clone();
        rot.rotate_left(k);
        assert_eq!(strongest(rot.clone()), expect, "rotation {k}");
        rot.reverse();
        assert_eq!(strongest(rot), expect, "reversed rotation {k}");
    }
    assert_eq!(expect, m(Classification::Csam, MatchType::Exact));
}

#[test]
fn an_empty_set_of_answers_is_not_clean() {
    assert_eq!(
        strongest(Vec::new()),
        MatchOutcome::Unavailable(Unavailable::NotConfigured)
    );
}

// ---- request and caps ------------------------------------------------------------------------

#[test]
fn request_takes_one_to_eight_hashes() {
    assert_eq!(MatchRequest::pdq(vec![]).unwrap_err(), RequestError::Empty);
    assert!(MatchRequest::pdq(vec![Hash256::zero(); 8]).is_ok());
    assert_eq!(
        MatchRequest::pdq(vec![Hash256::zero(); 9]).unwrap_err(),
        RequestError::TooMany { given: 9 }
    );
}

#[test]
fn detail_fields_and_counts_are_capped_on_a_character_boundary() {
    let long = "é".repeat(MAX_DETAIL_FIELD_BYTES); // 2 bytes each
    let d = MatchDetail::new(
        vec![Digest {
            algorithm: long.clone(),
            value: long,
        }],
        Some("x".repeat(1000)),
    );
    assert!(d.digests[0].algorithm.len() <= MAX_DETAIL_FIELD_BYTES);
    assert!(
        d.digests[0]
            .value
            .is_char_boundary(d.digests[0].value.len())
    );
    assert_eq!(d.listed_at.as_ref().unwrap().len(), MAX_DETAIL_FIELD_BYTES);
    let k = KnownMatch::new(
        Classification::Csam,
        MatchType::Exact,
        vec![MatchDetail::default(); MAX_DETAILS + 10],
    );
    assert_eq!(k.details.len(), MAX_DETAILS);
}

#[test]
fn status_classification_never_yields_clean_and_matches_the_documented_map() {
    assert_eq!(classify_status(200), None);
    assert_eq!(classify_status(204), None);
    assert_eq!(classify_status(401), Some(Unavailable::CredentialsRefused));
    assert_eq!(classify_status(403), Some(Unavailable::CredentialsRefused));
    assert_eq!(classify_status(408), Some(Unavailable::Timeout));
    assert_eq!(classify_status(504), Some(Unavailable::Timeout));
    assert_eq!(classify_status(429), Some(Unavailable::Throttled));
    assert_eq!(classify_status(500), Some(Unavailable::ServerError));
    assert_eq!(classify_status(503), Some(Unavailable::ServerError));
    assert_eq!(classify_status(404), Some(Unavailable::MalformedResponse));
    assert_eq!(classify_status(302), Some(Unavailable::MalformedResponse));
    for s in 0..=999u16 {
        if !(200..=299).contains(&s) {
            assert!(classify_status(s).is_some(), "status {s}");
        }
    }
}

#[test]
fn body_cap_is_inclusive() {
    assert!(within_cap(&[0; 10], 10));
    assert!(!within_cap(&[0; 11], 10));
}

// ---- retry ------------------------------------------------------------------------------------

#[test]
fn retry_is_once_transient_only_and_inside_the_deadline() {
    let p = RetryPolicy::default();
    let go = RetryDecision::Retry { after_ms: 250 };
    assert_eq!(p.decide(1, Unavailable::Timeout, 100, 5_000), go);
    assert_eq!(p.decide(1, Unavailable::ServerError, 100, 5_000), go);
    // Second failure: no more retries.
    assert_eq!(
        p.decide(2, Unavailable::Timeout, 400, 5_000),
        RetryDecision::Stop
    );
    // Never for throttle, credentials, malformed, test value.
    for r in [
        Unavailable::Throttled,
        Unavailable::CredentialsRefused,
        Unavailable::MalformedResponse,
        Unavailable::TestValueInProduction,
        Unavailable::NotConfigured,
        Unavailable::Disabled,
    ] {
        assert_eq!(p.decide(1, r, 0, 5_000), RetryDecision::Stop, "{r:?}");
    }
    // The delay must still fit before the deadline.
    assert_eq!(
        p.decide(1, Unavailable::Timeout, 4_800, 5_000),
        RetryDecision::Stop
    );
    assert_eq!(
        p.decide(1, Unavailable::Timeout, u64::MAX, 5_000),
        RetryDecision::Stop
    );
}

// ---- mock -------------------------------------------------------------------------------------

#[test]
fn mock_answers_in_order_repeats_the_last_and_records_requests() {
    let mock = MockMatcher::new(vec![
        clean(),
        MatchOutcome::Unavailable(Unavailable::Timeout),
    ]);
    assert!(mock.lookup(&req()).is_clean());
    assert_eq!(
        mock.lookup(&req()),
        MatchOutcome::Unavailable(Unavailable::Timeout)
    );
    assert_eq!(
        mock.lookup(&req()),
        MatchOutcome::Unavailable(Unavailable::Timeout)
    );
    assert_eq!(mock.requests().len(), 3);
}

#[test]
fn an_empty_mock_is_not_configured_never_clean() {
    let mock = MockMatcher::new(vec![]);
    assert_eq!(
        mock.lookup(&req()),
        MatchOutcome::Unavailable(Unavailable::NotConfigured)
    );
    assert_eq!(
        MockMatcher::default().lookup(&req()),
        MatchOutcome::Unavailable(Unavailable::NotConfigured)
    );
}

// ---- conformance suite ------------------------------------------------------------------------

/// A toy provider whose "wire format" is the literal body text. It exists to test the suite.
struct Toy;

impl ProviderProtocol for Toy {
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
            b"CLEAN" => MatchOutcome::Clean(CleanBasis::default()),
            b"MATCH" => m(Classification::Csam, MatchType::Exact),
            // "no match, near" is self-contradictory.
            b"CLEAN-NEAR" => MatchOutcome::Unavailable(Unavailable::MalformedResponse),
            b"TEST" => MatchOutcome::Unavailable(Unavailable::TestValueInProduction),
            _ => MatchOutcome::Unavailable(Unavailable::MalformedResponse),
        }
    }
}

/// Fails closed on nothing: anything unparseable is clean. The suite must catch this.
struct Lax;

impl ProviderProtocol for Lax {
    fn request(&self, r: &MatchRequest) -> Result<HttpRequestSpec, ProtocolError> {
        Toy.request(r)
    }

    fn interpret(&self, r: &MatchRequest, resp: &HttpResponseSpec) -> MatchOutcome {
        match Toy.interpret(r, resp) {
            MatchOutcome::Unavailable(Unavailable::MalformedResponse) => {
                MatchOutcome::Clean(CleanBasis::default())
            }
            other => other,
        }
    }
}

fn case(name: &str, body: &[u8], expect: Expect) -> Case {
    Case {
        name: name.into(),
        response: HttpResponseSpec {
            status: 200,
            body: body.to_vec(),
        },
        expect,
    }
}

#[test]
fn a_fail_closed_profile_passes_the_suite() {
    let cases = vec![
        case("clean", b"CLEAN", Expect::Clean),
        case("match", b"MATCH", Expect::AnyMatch),
        case(
            "no match carrying a match type",
            b"CLEAN-NEAR",
            Expect::Unavailable(Unavailable::MalformedResponse),
        ),
        case(
            "test value",
            b"TEST",
            Expect::Unavailable(Unavailable::TestValueInProduction),
        ),
    ];
    assert_eq!(conformance_failures(&Toy, &cases), Vec::<String>::new());
}

#[test]
fn a_profile_that_reads_garbage_as_clean_fails_the_suite() {
    let failures = conformance_failures(&Lax, &[]);
    assert!(!failures.is_empty());
    assert!(
        failures
            .iter()
            .any(|f| f.contains("junk body") || f.contains("oversized")),
        "{failures:?}"
    );
}

#[test]
fn the_suite_reports_a_wrong_expectation() {
    let wrong = vec![case("clean is not a match", b"CLEAN", Expect::AnyMatch)];
    let failures = conformance_failures(&Toy, &wrong);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("clean is not a match"));
}

#[test]
fn request_spec_carries_no_credentials_field() {
    // The spec is plain data: method, path, headers, body. A profile that put a secret in a
    // header would be visible here; the generic type has no place for one by construction.
    let spec = Toy.request(&req()).unwrap();
    assert!(spec.headers.is_empty());
    assert_eq!(spec.body.len(), 32);
}
