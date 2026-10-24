// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::codec::{decode_outcome, decode_outcomes, encode_outcome, encode_outcomes};
use safeimage::matcher::{
    Classification, CleanBasis, Digest, KnownMatch, MatchDetail, MatchOutcome, MatchType,
    Unavailable, strongest,
};

fn clean() -> MatchOutcome {
    MatchOutcome::Clean(CleanBasis {
        list_version: Some("v7".into()),
        checked_at: Some(1_700_000_000),
    })
}

fn known(c: Classification, t: MatchType) -> MatchOutcome {
    MatchOutcome::Match(KnownMatch::new(
        c,
        t,
        vec![MatchDetail::new(
            vec![Digest {
                algorithm: "sha256".into(),
                value: "ab\u{e9}".into(),
            }],
            Some("2026-01-01".into()),
        )],
    ))
}

fn samples() -> Vec<MatchOutcome> {
    vec![
        clean(),
        MatchOutcome::Clean(CleanBasis::default()),
        MatchOutcome::Unavailable(Unavailable::Timeout),
        MatchOutcome::Unavailable(Unavailable::TestValueInProduction),
        known(Classification::HarmfulAbusive, MatchType::Near),
        known(Classification::Csam, MatchType::Exact),
        known(Classification::Other("x".into()), MatchType::Near),
        MatchOutcome::Match(KnownMatch::new(
            Classification::Csam,
            MatchType::Near,
            vec![],
        )),
    ]
}

#[test]
fn outcomes_round_trip() {
    for o in samples() {
        assert_eq!(decode_outcome(&encode_outcome(&o)).unwrap(), o);
    }
    let all = samples();
    assert_eq!(decode_outcomes(&encode_outcomes(&all)).unwrap(), all);
}

#[test]
fn strongest_matches_the_rust_api() {
    let all = samples();
    let want = strongest(all.clone());
    let got = strongest_bytes(&encode_outcomes(&all), 1 << 16).unwrap();
    assert_eq!(decode_outcome(&got).unwrap(), want);
    // A match outranks everything; the order of the list does not matter.
    let mut rev = all.clone();
    rev.reverse();
    assert_eq!(
        strongest_bytes(&encode_outcomes(&rev), 1 << 16).unwrap(),
        got
    );
}

#[test]
fn empty_list_is_unavailable() {
    let got = strongest_bytes(&encode_outcomes(&[]), 64).unwrap();
    assert_eq!(
        decode_outcome(&got).unwrap(),
        MatchOutcome::Unavailable(Unavailable::NotConfigured)
    );
}

#[test]
fn unknown_outcome_tag_is_never_clean() {
    // version, count 2, a clean outcome, then an outcome tag 9.
    let mut bytes = encode_outcomes(&[clean()]);
    bytes[1] = 2;
    bytes.push(9);
    let got = decode_outcome(&strongest_bytes(&bytes, 64).unwrap()).unwrap();
    assert_eq!(
        got,
        MatchOutcome::Unavailable(Unavailable::MalformedResponse)
    );
    // An unknown unavailable reason is also unavailable.
    let bytes = [codec::VERSION, 1, 0, 0, 0, 1, 200];
    let got = decode_outcome(&strongest_bytes(&bytes, 64).unwrap()).unwrap();
    assert_eq!(
        got,
        MatchOutcome::Unavailable(Unavailable::MalformedResponse)
    );
}

#[test]
fn unknown_classification_or_match_type_is_a_refusal() {
    let mut b = encode_outcomes(&[known(Classification::Csam, MatchType::Exact)]);
    // layout: version, count(4), outcome tag, classification tag, match type
    b[6] = 77;
    assert_eq!(strongest_bytes(&b, 1 << 12), Err(ERR_UNKNOWN_TAG));
    let mut b = encode_outcomes(&[known(Classification::Csam, MatchType::Exact)]);
    b[7] = 77;
    assert_eq!(strongest_bytes(&b, 1 << 12), Err(ERR_UNKNOWN_TAG));
}

#[test]
fn every_truncation_is_an_error_and_never_a_panic() {
    let full = encode_outcomes(&samples());
    for n in 0..full.len() {
        assert!(strongest_bytes(&full[..n], 1 << 16).is_err(), "prefix {n}");
    }
    assert!(strongest_bytes(&full, 1 << 16).is_ok());
    let mut extra = full.clone();
    extra.push(0);
    assert_eq!(strongest_bytes(&extra, 1 << 16), Err(ERR_TRAILING_BYTES));
    let mut v = full.clone();
    v[0] = 9;
    assert_eq!(strongest_bytes(&v, 1 << 16), Err(ERR_BAD_VERSION));
}

#[test]
fn hostile_counts_do_not_allocate_or_panic() {
    // A count of u32::MAX followed by nothing: refused as truncated, no giant allocation.
    let bytes = [codec::VERSION, 255, 255, 255, 255];
    assert_eq!(strongest_bytes(&bytes, 64), Err(ERR_TRUNCATED));
    let bytes = [
        codec::VERSION,
        1,
        0,
        0,
        0,
        OUT_MATCH_TAG,
        0,
        255,
        255,
        255,
        255,
    ];
    assert!(strongest_bytes(&bytes, 64).is_err());
}

const OUT_MATCH_TAG: u8 = 2;

#[test]
fn input_cap_and_output_cap() {
    let big = vec![0u8; MAX_INPUT_BYTES + 1];
    assert_eq!(strongest_bytes(&big, 64), Err(ERR_INPUT_TOO_LARGE));
    let bytes = encode_outcomes(&[clean()]);
    assert_eq!(strongest_bytes(&bytes, 1), Err(ERR_OUT_TOO_SMALL));
}

#[test]
fn invalid_utf8_is_malformed() {
    let mut b = encode_outcomes(&[known(Classification::Other("ab".into()), MatchType::Near)]);
    // outcome tag at 5, classification tag at 6, string length at 7..11, bytes at 11..13
    b[11] = 0xff;
    assert_eq!(strongest_bytes(&b, 1 << 12), Err(ERR_MALFORMED));
}

#[test]
fn status_classification_matches() {
    assert_eq!(si_classify_status(200), 0);
    assert_eq!(si_classify_status(299), 0);
    assert_eq!(si_classify_status(401), 1 + 5);
    assert_eq!(si_classify_status(408), 1 + 2);
    assert_eq!(si_classify_status(429), 1 + 4);
    assert_eq!(si_classify_status(503), 1 + 3);
    assert_eq!(si_classify_status(302), 1 + 6);
    assert_eq!(si_classify_status(70_000), 1 + 6);
    assert_eq!(si_within_cap(10, 10), 1);
    assert_eq!(si_within_cap(11, 10), 0);
}

#[test]
fn retry_matches_the_policy() {
    let timeout = 2;
    let r = retry_bytes(1, 250, 1, timeout, 100, 1000);
    assert_eq!(r[0], 1);
    assert_eq!(u64::from_le_bytes(r[1..].try_into().unwrap()), 250);
    // A second failure, a throttle, an unknown tag and a missed deadline all stop.
    assert_eq!(retry_bytes(1, 250, 2, timeout, 100, 1000)[0], 0);
    assert_eq!(retry_bytes(1, 250, 1, 4, 100, 1000)[0], 0);
    assert_eq!(retry_bytes(1, 250, 1, 999, 100, 1000)[0], 0);
    assert_eq!(retry_bytes(1, 250, 1, timeout, 800, 1000)[0], 0);
    assert_eq!(retry_bytes(1, 250, 1, timeout, u64::MAX, u64::MAX)[0], 0);
}

#[test]
fn request_validation() {
    let mk = |n: u32| {
        let mut v = vec![codec::VERSION];
        v.extend_from_slice(&n.to_le_bytes());
        v.extend(core::iter::repeat_n(7u8, 32 * n as usize));
        v
    };
    assert_eq!(match_request_len(&mk(1)), Ok(1));
    assert_eq!(match_request_len(&mk(8)), Ok(8));
    assert_eq!(match_request_len(&mk(0)), Err(ERR_BAD_ARGUMENT));
    assert_eq!(match_request_len(&mk(9)), Err(ERR_BAD_ARGUMENT));
    let mut short = mk(2);
    short.pop();
    assert_eq!(match_request_len(&short), Err(ERR_TRUNCATED));
}

/// The vectors a TypeScript host replays (`wasm/test.mjs`). Regenerate with
/// `SAFEIMAGE_WRITE_VECTORS=1 cargo test -p safeimage-wasm vectors`.
#[test]
fn vectors_file_is_current() {
    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }
    let mut text = String::from("# layer 2 vectors. See vectors/README.md for each line kind.\n");
    let all = samples();
    let mut cases: Vec<(String, Vec<MatchOutcome>)> = vec![("empty".into(), vec![])];
    for (i, o) in all.iter().enumerate() {
        cases.push((format!("single{i}"), vec![o.clone()]));
    }
    cases.push(("all".into(), all.clone()));
    for (name, list) in cases {
        let input = encode_outcomes(&list);
        let out = strongest_bytes(&input, 1 << 16).unwrap();
        text.push_str(&format!("strongest {name} {} {}\n", hex(&input), hex(&out)));
    }
    let full = encode_outcomes(&all);
    for (name, input, code) in [
        ("truncated", full[..full.len() - 1].to_vec(), ERR_TRUNCATED),
        ("bad_version", [9u8, 0, 0, 0, 0].to_vec(), ERR_BAD_VERSION),
    ] {
        text.push_str(&format!("strongest_err {name} {} {code}\n", hex(&input)));
    }
    for status in [
        200u32, 204, 299, 301, 401, 403, 408, 429, 500, 503, 504, 70_000,
    ] {
        text.push_str(&format!(
            "classify {status} {}\n",
            si_classify_status(status)
        ));
    }
    for (len, cap) in [(0usize, 0usize), (10, 10), (11, 10)] {
        text.push_str(&format!(
            "within_cap {len} {cap} {}\n",
            si_within_cap(len, cap)
        ));
    }
    // (name, max_retries, delay_ms, attempts, reason tag, elapsed_ms, deadline_ms)
    for (name, max, delay, attempts, tag, elapsed, deadline) in [
        ("timeout_first", 1u32, 250u64, 1u32, 2u32, 100u64, 1000u64),
        ("server_error_first", 1, 250, 1, 3, 100, 1000),
        ("timeout_second", 1, 250, 2, 2, 100, 1000),
        ("throttled", 1, 250, 1, 4, 100, 1000),
        ("credentials", 1, 250, 1, 5, 100, 1000),
        ("unknown_tag", 1, 250, 1, 999, 100, 1000),
        ("past_deadline", 1, 250, 1, 2, 800, 1000),
        ("two_retries", 2, 50, 2, 2, 10, 1000),
    ] {
        text.push_str(&format!(
            "retry {name} {max} {delay} {attempts} {tag} {elapsed} {deadline} {}\n",
            hex(&retry_bytes(max, delay, attempts, tag, elapsed, deadline))
        ));
    }
    let req = |n: u32| {
        let mut v = vec![codec::VERSION];
        v.extend_from_slice(&n.to_le_bytes());
        for i in 0..n {
            v.extend(core::iter::repeat_n(i as u8, 32));
        }
        v
    };
    for (name, bytes) in [
        ("one", req(1)),
        ("eight", req(8)),
        ("zero", req(0)),
        ("nine", req(9)),
        ("truncated", req(2)[..40].to_vec()),
    ] {
        let r = match match_request_len(&bytes) {
            Ok(n) => n as i32,
            Err(c) => c,
        };
        text.push_str(&format!("match_request {name} {} {r}\n", hex(&bytes)));
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/layer2.txt");
    if std::env::var_os("SAFEIMAGE_WRITE_VECTORS").is_some() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors")).unwrap();
        std::fs::write(path, &text).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}
