// SPDX-License-Identifier: MIT OR Apache-2.0
//! Layer 5 and the screening join point.

mod support;

use safeimage::classifier::*;
use safeimage::hash::{Frame, QUALITY_FLOOR_DEFAULT};
use safeimage::matcher::*;
use safeimage::screening::*;

fn sig(score: f32) -> ClassifierSignal {
    ClassifierSignal::new(score, ModelId::new("m").unwrap(), None).unwrap()
}

fn clean() -> MatchOutcome {
    MatchOutcome::Clean(CleanBasis::default())
}

fn hit() -> MatchOutcome {
    MatchOutcome::Match(KnownMatch::new(
        Classification::Csam,
        MatchType::Near,
        vec![],
    ))
}

const REASONS: [Unavailable; 8] = [
    Unavailable::NotConfigured,
    Unavailable::Disabled,
    Unavailable::Timeout,
    Unavailable::ServerError,
    Unavailable::Throttled,
    Unavailable::CredentialsRefused,
    Unavailable::MalformedResponse,
    Unavailable::TestValueInProduction,
];

// ---- signal -------------------------------------------------------------------------------------

#[test]
fn a_score_must_be_a_finite_number_from_zero_to_one() {
    let m = || ModelId::new("m").unwrap();
    for ok in [0.0f32, 0.5, 1.0] {
        assert!(ClassifierSignal::new(ok, m(), None).is_ok());
    }
    for bad in [
        -0.0001f32,
        1.0001,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ] {
        assert_eq!(
            ClassifierSignal::new(bad, m(), None).unwrap_err(),
            SignalError::ScoreOutOfRange,
            "{bad}"
        );
    }
    assert_eq!(
        ClassifierSignal::new(0.5, m(), Some(1.5)).unwrap_err(),
        SignalError::ThresholdOutOfRange
    );
    assert_eq!(
        ClassifierSignal::new(0.5, m(), Some(f32::NAN)).unwrap_err(),
        SignalError::ThresholdOutOfRange
    );
    let s = ClassifierSignal::new(0.25, m(), Some(0.9)).unwrap();
    assert_eq!((s.score(), s.threshold_applied()), (0.25, Some(0.9)));
    assert_eq!(s.model().as_str(), "m");
}

#[test]
fn model_labels_are_bounded() {
    assert!(ModelId::new("").is_none());
    assert!(ModelId::new("x".repeat(65)).is_none());
    assert!(ModelId::new("x".repeat(64)).is_some());
}

#[test]
fn the_mock_classifier_follows_its_script_and_an_empty_one_is_unavailable() {
    let rgb = support::make(8, 8, 0);
    let f = Frame::new(&rgb, 8, 8).unwrap();
    let mock = MockClassifier::new(vec![Ok(sig(0.1)), Err(ClassifierError::BadAnswer)]);
    assert_eq!(mock.classify(&f).unwrap().score(), 0.1);
    assert_eq!(mock.classify(&f).unwrap_err(), ClassifierError::BadAnswer);
    assert_eq!(mock.classify(&f).unwrap_err(), ClassifierError::BadAnswer);
    assert_eq!(
        MockClassifier::new(vec![]).classify(&f).unwrap_err(),
        ClassifierError::Unavailable
    );
}

// ---- screening ----------------------------------------------------------------------------------

#[test]
fn only_a_clean_answer_allows_and_the_signal_comes_back_unmerged() {
    let rgb = support::make(300, 200, 4);
    let f = Frame::new(&rgb, 300, 200).unwrap();
    let p = ScreenPolicy::default();
    match screen(&f, clean(), Some(sig(0.7)), &p) {
        Screening::Allow { frame, classifier } => {
            assert_eq!(frame.frame().width(), 300);
            assert_eq!(classifier.unwrap().score(), 0.7);
        }
        other => panic!("expected Allow, got {other:?}"),
    }
}

#[test]
fn a_match_is_blocked_whatever_the_classifier_says() {
    let rgb = support::make(64, 64, 0);
    let f = Frame::new(&rgb, 64, 64).unwrap();
    let p = ScreenPolicy::default();
    for s in [None, Some(sig(0.0)), Some(sig(1.0))] {
        assert!(
            matches!(screen(&f, hit(), s, &p), Screening::Blocked(m) if m.classification == Classification::Csam),
        );
    }
}

#[test]
fn every_failure_is_a_hold_never_an_allow() {
    let rgb = support::make(64, 64, 0);
    let f = Frame::new(&rgb, 64, 64).unwrap();
    for r in REASONS {
        for s in [None, Some(sig(0.0))] {
            match screen(
                &f,
                MatchOutcome::Unavailable(r),
                s,
                &ScreenPolicy::default(),
            ) {
                Screening::Hold(ScreenHold::Unavailable(got)) => assert_eq!(got, r),
                other => panic!("{r:?}: expected Hold, got {other:?}"),
            }
        }
    }
}

#[test]
fn a_classifier_flag_holds_at_the_policy_level_and_only_there() {
    let rgb = support::make(64, 64, 0);
    let f = Frame::new(&rgb, 64, 64).unwrap();
    let p = ScreenPolicy {
        classifier_hold_at: Some(0.8),
        quality_floor: None,
    };
    assert!(matches!(
        screen(&f, clean(), Some(sig(0.8)), &p),
        Screening::Hold(ScreenHold::ClassifierFlag { score }) if score == 0.8
    ));
    assert!(matches!(
        screen(&f, clean(), Some(sig(0.79)), &p),
        Screening::Allow { .. }
    ));
    // No policy level: a score is passed along, never a hold.
    assert!(matches!(
        screen(&f, clean(), Some(sig(1.0)), &ScreenPolicy::default()),
        Screening::Allow { .. }
    ));
    // No signal: nothing to hold on.
    assert!(matches!(
        screen(&f, clean(), None, &p),
        Screening::Allow { .. }
    ));
}

#[test]
fn match_outcome_outranks_the_classifier_flag() {
    let rgb = support::make(64, 64, 0);
    let f = Frame::new(&rgb, 64, 64).unwrap();
    let p = ScreenPolicy {
        classifier_hold_at: Some(0.1),
        quality_floor: None,
    };
    assert!(matches!(
        screen(&f, hit(), Some(sig(0.9)), &p),
        Screening::Blocked(_)
    ));
    assert!(matches!(
        screen(
            &f,
            MatchOutcome::Unavailable(Unavailable::Timeout),
            Some(sig(0.9)),
            &p
        ),
        Screening::Hold(ScreenHold::Unavailable(Unavailable::Timeout))
    ));
}

#[test]
fn a_model_only_ever_receives_a_screened_frame() {
    // The "model-facing API": it cannot be called without a ScreenedFrame, and a ScreenedFrame
    // can only come from `screen` returning Allow.
    fn run_model(calls: &mut u32, f: &ScreenedFrame<'_, '_>) -> u32 {
        *calls += 1;
        f.frame().width() * f.frame().height()
    }
    let rgb = support::make(32, 32, 1);
    let f = Frame::new(&rgb, 32, 32).unwrap();
    let p = ScreenPolicy::default();
    let mut calls = 0u32;
    let mut outcomes: Vec<MatchOutcome> = REASONS
        .iter()
        .map(|r| MatchOutcome::Unavailable(*r))
        .collect();
    outcomes.push(hit());
    outcomes.push(clean());
    outcomes.push(clean());
    for o in outcomes {
        if let Screening::Allow { frame, .. } = screen(&f, o, None, &p) {
            assert_eq!(run_model(&mut calls, &frame), 32 * 32);
        }
    }
    assert_eq!(
        calls, 2,
        "the model ran once per clean answer and never otherwise"
    );
}

#[test]
fn a_frame_too_flat_to_hash_is_held_before_any_provider_call() {
    let flat = support::make(200, 200, 3);
    let f = Frame::new(&flat, 200, 200).unwrap();
    let got = match_request(&f, &ScreenPolicy::default());
    assert!(
        matches!(got, Err(ScreenHold::LowQuality { quality }) if quality <= QUALITY_FLOOR_DEFAULT)
    );
}

#[test]
fn a_mid_quality_frame_is_held_by_the_default_floor_and_passes_a_lower_one() {
    // Quality strictly between 0 and the default floor of 49 (the synthetic "soft blocks" frame).
    let rgb = support::make(200, 150, 6);
    let f = Frame::new(&rgb, 200, 150).unwrap();
    let q = safeimage::hash::pdq_dihedral(&f).quality;
    assert!(q > 0 && q <= QUALITY_FLOOR_DEFAULT, "fixture quality {q}");
    assert!(matches!(
        match_request(&f, &ScreenPolicy::default()),
        Err(ScreenHold::LowQuality { quality }) if quality == q
    ));
    let lower = ScreenPolicy {
        classifier_hold_at: None,
        quality_floor: Some(0),
    };
    assert!(match_request(&f, &lower).is_ok());
}

#[test]
fn a_hashable_frame_makes_a_request_of_eight_orientations() {
    let rgb = support::make(512, 512, 1);
    let f = Frame::new(&rgb, 512, 512).unwrap();
    let r = match_request(&f, &ScreenPolicy::default()).unwrap();
    assert_eq!(r.hashes().len(), 8);
}

#[test]
fn the_quality_floor_is_the_policys_when_given() {
    let rgb = support::make(512, 512, 1); // quality 100
    let f = Frame::new(&rgb, 512, 512).unwrap();
    let strict = ScreenPolicy {
        classifier_hold_at: None,
        quality_floor: Some(100),
    };
    assert!(matches!(
        match_request(&f, &strict),
        Err(ScreenHold::LowQuality { quality: 100 })
    ));
    let lax = ScreenPolicy {
        classifier_hold_at: None,
        quality_floor: Some(99),
    };
    assert!(match_request(&f, &lax).is_ok());
}

// ---- layer boundaries (I3, I4) -------------------------------------------------------------------

#[test]
fn the_classifier_module_imports_only_the_frame_type() {
    let src = include_str!("../src/classifier/mod.rs");
    let uses: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("use ") && !l.starts_with("//"))
        .collect();
    assert!(!uses.is_empty());
    for u in &uses {
        assert!(
            !u.contains("matcher")
                && !u.contains("report")
                && !u.contains("evidence")
                && !u.contains("screening"),
            "classifier module imports from another layer: {u}"
        );
    }
    assert!(uses.iter().any(|u| u.contains("crate::hash::Frame")));
}

#[test]
fn no_conversion_exists_between_signals_and_outcomes() {
    for path in [
        include_str!("../src/classifier/mod.rs"),
        include_str!("../src/matcher/outcome.rs"),
        include_str!("../src/screening/mod.rs"),
    ] {
        // An `impl` item (not an `impl Trait` argument) of From or Into.
        for line in path.lines().map(str::trim_start) {
            let is_item = line.starts_with("impl") && line.contains(" for ");
            assert!(
                !(is_item && (line.contains("From<") || line.contains("Into<"))),
                "a conversion impl exists: {line}"
            );
        }
    }
}
