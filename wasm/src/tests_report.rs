// SPDX-License-Identifier: MIT OR Apache-2.0
use super::*;
use crate::report_codec::{decode_state, decode_step, encode_answer, encode_state, encode_step};
use safeimage::report::{DestinationAnswer, Event, ReportState, Status, Step, apply, next_step};

const BIG: usize = 1 << 16;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn answer(a: DestinationAnswer) -> Vec<u8> {
    encode_answer(&a)
}

fn accepted(id: Option<&str>) -> DestinationAnswer {
    DestinationAnswer::Accepted {
        report_id: id.map(String::from),
    }
}

/// Runs `apply` through the byte API and checks it equals the Rust API.
fn apply_bytes(
    s: &ReportState,
    step: &Step,
    a: &DestinationAnswer,
    now: u64,
) -> (ReportState, Option<Event>) {
    let (want_state, want_event) = apply(s, step, a, now);
    let got = report_apply_bytes(
        &encode_state(s),
        &encode_step(step),
        &answer(a.clone()),
        now,
        encode_state(s).len() + REPORT_APPLY_HEADROOM,
    )
    .unwrap();
    assert_eq!(
        got,
        crate::report_codec::encode_applied(&want_state, &want_event)
    );
    (want_state, want_event)
}

#[test]
fn state_step_and_answer_round_trip() {
    let mut s = ReportState::new(2, true);
    s.report_id = Some("r-1".into());
    s.last_code = Some("transient".into());
    s.abandoned_ids = vec!["old".into()];
    s.next_attempt_at = Some(42);
    s.files[1].uploaded = true;
    assert_eq!(decode_state(&encode_state(&s)).unwrap(), s);
    for st in [
        Step::Submit,
        Step::Upload { file: 3 },
        Step::Describe { file: 0 },
        Step::Finish,
        Step::Retract,
        Step::Wait { until: 9 },
        Step::Hold(safeimage::report::HoldReason::Failed),
        Step::Hold(safeimage::report::HoldReason::AwaitingCredentials),
        Step::Done,
    ] {
        assert_eq!(decode_step(&encode_step(&st)).unwrap(), st);
    }
}

#[test]
fn full_lifecycle_through_bytes_matches_the_rust_api() {
    let mut s = ReportState::new(2, true);
    let mut steps = 0;
    loop {
        let step = next_step(&s, 0);
        // The byte API agrees on what is due.
        let due = report_next_step_bytes(&encode_state(&s), 0, BIG).unwrap();
        assert_eq!(due, encode_step(&step));
        if matches!(step, Step::Done) {
            break;
        }
        let a = if matches!(step, Step::Submit) {
            accepted(Some("rid"))
        } else {
            accepted(None)
        };
        let (n, ev) = apply_bytes(&s, &step, &a, 0);
        assert!(ev.is_none());
        s = n;
        steps += 1;
        assert!(steps < 20, "lifecycle must terminate");
    }
    assert_eq!(s.status, Status::Finished);
    assert_eq!(steps, 6); // submit, 2 x (upload, describe), finish
}

#[test]
fn failures_never_advance() {
    let s = ReportState::new(1, true);
    for a in [
        DestinationAnswer::Transient,
        DestinationAnswer::AuthRefused,
        DestinationAnswer::Unknown("x".into()),
    ] {
        let (n, _) = apply_bytes(&s, &Step::Submit, &a, 100);
        assert_eq!(n.status, Status::Pending);
        assert_eq!(n.attempts, 1);
        assert!(n.next_attempt_at.is_some());
    }
    // An accepted submit without an id is handled as transient.
    let (n, ev) = apply_bytes(&s, &Step::Submit, &accepted(None), 0);
    assert_eq!(n.status, Status::Pending);
    assert_eq!(ev, Some(Event::MissingReportId));
}

#[test]
fn unknown_answer_tag_is_transient_never_success() {
    let s = ReportState::new(1, true);
    let bytes = [codec::VERSION, 99];
    let got = report_apply_bytes(
        &encode_state(&s),
        &encode_step(&Step::Submit),
        &bytes,
        0,
        BIG,
    )
    .unwrap();
    let (want, ev) = apply(
        &s,
        &Step::Submit,
        &DestinationAnswer::Unknown("unknown_tag".into()),
        0,
    );
    assert_eq!(want.status, Status::Pending);
    assert_eq!(got, crate::report_codec::encode_applied(&want, &ev));
}

#[test]
fn unknown_state_or_step_tag_is_a_refusal() {
    let mut s = encode_state(&ReportState::new(1, true));
    s[1] = 77; // status tag
    assert_eq!(report_next_step_bytes(&s, 0, BIG), Err(ERR_UNKNOWN_TAG));
    let state = encode_state(&ReportState::new(1, true));
    assert_eq!(
        report_apply_bytes(
            &state,
            &[codec::VERSION, 77],
            &encode_answer(&DestinationAnswer::Transient),
            0,
            BIG
        ),
        Err(ERR_UNKNOWN_TAG)
    );
}

#[test]
fn every_truncation_is_an_error() {
    let mut s = ReportState::new(3, true);
    s.report_id = Some("rid".into());
    s.last_code = Some("c".into());
    s.abandoned_ids = vec!["a".into(), "b".into()];
    let full = encode_state(&s);
    for n in 0..full.len() {
        assert!(
            report_next_step_bytes(&full[..n], 0, BIG).is_err(),
            "state prefix {n}"
        );
        assert!(report_sync_bytes(&full[..n], 1, BIG).is_err());
        assert!(report_retract_bytes(&full[..n], BIG).is_err());
        assert!(report_retry_bytes(&full[..n], BIG).is_err());
    }
    let step = encode_step(&Step::Upload { file: 1 });
    let ans = encode_answer(&DestinationAnswer::Invalid("detail".into()));
    for n in 0..step.len() {
        assert!(report_apply_bytes(&full, &step[..n], &ans, 0, BIG).is_err());
    }
    for n in 0..ans.len() {
        assert!(
            report_apply_bytes(&full, &step, &ans[..n], 0, BIG).is_err(),
            "answer prefix {n}"
        );
    }
}

#[test]
fn argument_and_size_limits() {
    assert_eq!(
        report_new_bytes(MAX_REPORT_FILES + 1, 1, BIG),
        Err(ERR_BAD_ARGUMENT)
    );
    assert_eq!(report_new_bytes(1, 2, BIG), Err(ERR_BAD_ARGUMENT));
    assert_eq!(report_new_bytes(1, 1, 1), Err(ERR_OUT_TOO_SMALL));
    let s = encode_state(&ReportState::new(1, true));
    assert_eq!(report_sync_bytes(&s, 2, BIG), Err(ERR_BAD_ARGUMENT));
    assert_eq!(
        report_next_step_bytes(&vec![0u8; MAX_INPUT_BYTES + 1], 0, BIG),
        Err(ERR_INPUT_TOO_LARGE)
    );
    // A hostile file count with no bytes behind it is truncated, not allocated.
    let mut hostile = vec![codec::VERSION, 1, 0, 0, 0, 0, 0, 0];
    hostile.extend_from_slice(&[255, 255, 255, 255]);
    assert_eq!(report_next_step_bytes(&hostile, 0, BIG), Err(ERR_TRUNCATED));
}

#[test]
fn retract_retry_sync_match_the_rust_api() {
    let pending = ReportState::new(1, true);
    let out = report_retract_bytes(&encode_state(&pending), BIG).unwrap();
    assert_eq!(decode_state(&out).unwrap().status, Status::Withdrawn);

    let mut finished = pending.clone();
    finished.status = Status::Finished;
    assert_eq!(
        report_retract_bytes(&encode_state(&finished), BIG),
        Err(ERR_ALREADY_FINISHED)
    );

    let mut failed = pending.clone();
    failed.status = Status::Failed;
    failed.report_id = Some("rid".into());
    let out = report_retry_bytes(&encode_state(&failed), BIG).unwrap();
    let back = decode_state(&out).unwrap();
    assert_eq!(back.status, Status::Pending);
    assert_eq!(back.abandoned_ids, vec!["rid".to_string()]);

    let waiting = ReportState::new(1, false);
    let out = report_sync_bytes(&encode_state(&waiting), 1, BIG).unwrap();
    assert_eq!(decode_state(&out).unwrap().status, Status::Pending);
}

#[test]
fn scalar_exports_match() {
    for n in 0..8 {
        assert_eq!(si_backoff_secs(n), safeimage::report::backoff_secs(n));
    }
    assert_eq!(si_deletion_window_elapsed(0, 0, 100, 50, 60), 1);
    assert_eq!(si_deletion_window_elapsed(0, 0, 60, 50, 60), 0);
}

/// Layer 4 vectors for a TypeScript host. Regenerate with
/// `SAFEIMAGE_WRITE_VECTORS=1 cargo test -p safeimage-wasm vectors`.
#[test]
fn layer4_vectors_file_is_current() {
    let mut t = String::from(
        "# layer 4 vectors. new <name> <files> <ready> <out> | next <name> <state> <now> <out>\n\
         # apply <name> <state> <step> <answer> <now> <out> | sync <name> <state> <ready> <out>\n\
         # retract|retry <name> <state> <out or code> | backoff <n> <secs> | window <opened> <modified> <now> <open> <idle> <0|1>\n",
    );
    let ready = ReportState::new(1, true);
    let waiting = ReportState::new(1, false);
    t += &format!(
        "new ready {} 1 {}\n",
        1,
        hex(&report_new_bytes(1, 1, BIG).unwrap())
    );
    t += &format!(
        "new waiting {} 0 {}\n",
        1,
        hex(&report_new_bytes(1, 0, BIG).unwrap())
    );
    let mut s = ready.clone();
    let mut i = 0;
    loop {
        let step = next_step(&s, 0);
        t += &format!(
            "next s{i} {} 0 {}\n",
            hex(&encode_state(&s)),
            hex(&encode_step(&step))
        );
        if matches!(step, Step::Done) {
            break;
        }
        let a = if matches!(step, Step::Submit) {
            accepted(Some("rid-1"))
        } else {
            accepted(None)
        };
        let out = report_apply_bytes(
            &encode_state(&s),
            &encode_step(&step),
            &answer(a.clone()),
            0,
            BIG,
        )
        .unwrap();
        t += &format!(
            "apply a{i} {} {} {} 0 {}\n",
            hex(&encode_state(&s)),
            hex(&encode_step(&step)),
            hex(&answer(a.clone())),
            hex(&out)
        );
        s = apply(&s, &step, &a, 0).0;
        i += 1;
    }
    for (name, a) in [
        ("transient", DestinationAnswer::Transient),
        ("auth", DestinationAnswer::AuthRefused),
        ("invalid", DestinationAnswer::Invalid("bad".into())),
        ("window", DestinationAnswer::WindowExpired),
        ("unknown", DestinationAnswer::Unknown("zz".into())),
    ] {
        let out = report_apply_bytes(
            &encode_state(&ready),
            &encode_step(&Step::Submit),
            &answer(a.clone()),
            1000,
            BIG,
        )
        .unwrap();
        t += &format!(
            "apply f_{name} {} {} {} 1000 {}\n",
            hex(&encode_state(&ready)),
            hex(&encode_step(&Step::Submit)),
            hex(&answer(a)),
            hex(&out)
        );
    }
    t += &format!(
        "sync up {} 1 {}\n",
        hex(&encode_state(&waiting)),
        hex(&report_sync_bytes(&encode_state(&waiting), 1, BIG).unwrap())
    );
    t += &format!(
        "retract pending {} {}\n",
        hex(&encode_state(&ready)),
        hex(&report_retract_bytes(&encode_state(&ready), BIG).unwrap())
    );
    t += &format!(
        "retract finished {} {}\n",
        hex(&encode_state(&s)),
        ERR_ALREADY_FINISHED
    );
    let mut failed = ready.clone();
    failed.status = Status::Failed;
    t += &format!(
        "retry failed {} {}\n",
        hex(&encode_state(&failed)),
        hex(&report_retry_bytes(&encode_state(&failed), BIG).unwrap())
    );
    for n in [0u32, 1, 2, 3, 4, 5, 6, 40] {
        t += &format!("backoff {n} {}\n", si_backoff_secs(n));
    }
    t += &format!(
        "window 0 0 100 50 60 {}\n",
        si_deletion_window_elapsed(0, 0, 100, 50, 60)
    );
    t += &format!(
        "window 0 0 60 50 60 {}\n",
        si_deletion_window_elapsed(0, 0, 60, 50, 60)
    );

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/layer4.txt");
    if std::env::var_os("SAFEIMAGE_WRITE_VECTORS").is_some() {
        std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/vectors")).unwrap();
        std::fs::write(path, &t).unwrap();
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), t);
}
