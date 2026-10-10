//! A suite a provider profile runs against itself. It lives here so every profile is held to the
//! same fail-closed rules without this repository knowing any provider's wire format.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use super::outcome::{MatchOutcome, Unavailable};
use super::protocol::{
    HttpResponseSpec, MAX_RESPONSE_BYTES, MatchRequest, ProviderProtocol, classify_status,
};
use crate::hash::Hash256;

/// What a profile expects of one of its own recorded responses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expect {
    /// A known match of any kind.
    AnyMatch,
    /// `Clean`.
    Clean,
    /// `Unavailable` with this reason.
    Unavailable(Unavailable),
}

/// A profile's own recorded case: a response shape written from its public API document (never a
/// live call) and what it must interpret to.
#[derive(Debug, Clone)]
pub struct Case {
    /// Name for failure messages.
    pub name: String,
    /// The response.
    pub response: HttpResponseSpec,
    /// The expected outcome.
    pub expect: Expect,
}

fn check(expect: &Expect, got: &MatchOutcome) -> bool {
    match (expect, got) {
        (Expect::AnyMatch, MatchOutcome::Match(_)) => true,
        (Expect::Clean, MatchOutcome::Clean(_)) => true,
        (Expect::Unavailable(a), MatchOutcome::Unavailable(b)) => a == b,
        _ => false,
    }
}

/// Runs the generic fail-closed rules plus the profile's own `cases`, and returns one message per
/// failure (empty means conformant). The generic rules:
///
/// 1. every non-2xx status interprets to exactly what [`classify_status`] says, never `Clean`;
/// 2. a 2xx response over [`MAX_RESPONSE_BYTES`] is `Unavailable(MalformedResponse)`;
/// 3. an empty 2xx body and bodies of arbitrary bytes are always `Unavailable`, never `Clean`
///    or `Match`;
/// 4. interpreting never panics over a sweep of short and structured byte strings.
pub fn conformance_failures<P: ProviderProtocol>(profile: &P, cases: &[Case]) -> Vec<String> {
    let mut failures = Vec::new();
    let req = MatchRequest::pdq(vec![Hash256::zero()]).expect("one hash is a valid request");

    for status in [
        400u16, 401, 403, 404, 408, 429, 500, 502, 503, 504, 301, 302, 100,
    ] {
        let want = classify_status(status).expect("non-2xx has a reason");
        let got = profile.interpret(
            &req,
            &HttpResponseSpec {
                status,
                body: b"{}".to_vec(),
            },
        );
        if got != MatchOutcome::Unavailable(want) {
            failures.push(format!(
                "status {status}: expected Unavailable({want:?}), got {got:?}"
            ));
        }
    }

    let big = HttpResponseSpec {
        status: 200,
        body: vec![b' '; MAX_RESPONSE_BYTES + 1],
    };
    let got = profile.interpret(&req, &big);
    if got != MatchOutcome::Unavailable(Unavailable::MalformedResponse) {
        failures.push(format!(
            "oversized body: expected MalformedResponse, got {got:?}"
        ));
    }

    let mut junk: Vec<Vec<u8>> = vec![
        Vec::new(),
        b"null".to_vec(),
        b"{}".to_vec(),
        b"[]".to_vec(),
        b"{".to_vec(),
        b"\xff\xfe\x00".to_vec(),
        b"<html></html>".to_vec(),
        b"{\"classification\":".to_vec(),
    ];
    for n in 0..=40usize {
        junk.push((0..n).map(|i| (i * 31 % 251) as u8).collect());
    }
    for body in junk {
        let got = profile.interpret(
            &req,
            &HttpResponseSpec {
                status: 200,
                body: body.clone(),
            },
        );
        if !matches!(got, MatchOutcome::Unavailable(_)) {
            failures.push(format!(
                "junk body {body:?}: expected Unavailable, got {got:?}"
            ));
        }
    }

    for case in cases {
        let got = profile.interpret(&req, &case.response);
        if !check(&case.expect, &got) {
            failures.push(format!(
                "case {}: expected {:?}, got {got:?}",
                case.name, case.expect
            ));
        }
    }
    failures
}
