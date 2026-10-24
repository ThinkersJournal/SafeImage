// SPDX-License-Identifier: MIT OR Apache-2.0
//! Layer 5: where a model-based score enters, kept apart from known-hash matching.
//!
//! This module imports **only** the frame type from layer 1. It has no path to the matcher, the
//! evidence workflow or the report mechanism (design invariant I3: match data never trains,
//! evaluates or prompts a model), and a signal here has no conversion to or from a match outcome
//! (I4). A test reads this module's source to keep it that way.
//!
//! The crate ships no model. Classifiers are products behind [`Classifier`]; training or
//! validating a classifier for this material needs possession of the material itself, which no
//! project here may have.

use alloc::string::String;
use core::fmt;

use crate::hash::Frame;

/// Which model produced a signal, by the operator's own label.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelId(String);

impl ModelId {
    /// Wraps a non-empty label of at most 64 bytes.
    pub fn new(label: impl Into<String>) -> Option<Self> {
        let label = label.into();
        if label.is_empty() || label.len() > 64 {
            None
        } else {
            Some(ModelId(label))
        }
    }

    /// The label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A model's score for one frame. **Not** a known-match answer: a score is a reason to look,
/// never a determination, and nothing in this crate files a report from it.
///
/// There is no conversion between a signal and a match outcome, in either direction:
///
/// ```compile_fail,E0277
/// use safeimage::classifier::{ClassifierSignal, ModelId};
/// use safeimage::matcher::MatchOutcome;
/// let s = ClassifierSignal::new(0.9, ModelId::new("m").unwrap(), None).unwrap();
/// let _o: MatchOutcome = s.into();
/// ```
///
/// ```compile_fail,E0277
/// use safeimage::classifier::ClassifierSignal;
/// use safeimage::matcher::{CleanBasis, MatchOutcome};
/// let o = MatchOutcome::Clean(CleanBasis::default());
/// let _s: ClassifierSignal = o.into();
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ClassifierSignal {
    score: f32,
    model: ModelId,
    threshold_applied: Option<f32>,
}

/// Why a [`ClassifierSignal`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignalError {
    /// The score is NaN, infinite, or outside 0.0 to 1.0.
    ScoreOutOfRange,
    /// The threshold is NaN, infinite, or outside 0.0 to 1.0.
    ThresholdOutOfRange,
}

fn unit(v: f32) -> bool {
    v.is_finite() && (0.0..=1.0).contains(&v)
}

impl ClassifierSignal {
    /// A score in 0.0 to 1.0 from `model`. `threshold_applied` is the model's own cut-off, if it
    /// reports one, kept so a reviewer can see what the score was compared with.
    pub fn new(
        score: f32,
        model: ModelId,
        threshold_applied: Option<f32>,
    ) -> Result<Self, SignalError> {
        if !unit(score) {
            return Err(SignalError::ScoreOutOfRange);
        }
        if let Some(t) = threshold_applied
            && !unit(t)
        {
            return Err(SignalError::ThresholdOutOfRange);
        }
        Ok(ClassifierSignal {
            score,
            model,
            threshold_applied,
        })
    }

    /// The score, 0.0 to 1.0.
    pub fn score(&self) -> f32 {
        self.score
    }

    /// The model that produced it.
    pub fn model(&self) -> &ModelId {
        &self.model
    }

    /// The model's own threshold, if it reported one.
    pub fn threshold_applied(&self) -> Option<f32> {
        self.threshold_applied
    }
}

/// Why a classifier could not answer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClassifierError {
    /// The model is unavailable or timed out.
    Unavailable,
    /// The model answered something that is not a score.
    BadAnswer,
}

impl fmt::Display for ClassifierError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClassifierError::Unavailable => f.write_str("classifier unavailable"),
            ClassifierError::BadAnswer => f.write_str("classifier gave an invalid answer"),
        }
    }
}

/// A model behind the seam. It takes **only a frame**: there is deliberately no parameter through
/// which match data could reach it.
pub trait Classifier {
    /// Scores one frame.
    fn classify(&self, frame: &Frame<'_>) -> Result<ClassifierSignal, ClassifierError>;
}

/// Scripted answers for tests and for consumers' own tests. Answers repeat the last one.
#[derive(Debug)]
pub struct MockClassifier {
    script: alloc::vec::Vec<Result<ClassifierSignal, ClassifierError>>,
    next: core::cell::Cell<usize>,
}

impl MockClassifier {
    /// A mock answering with `script` in order; an empty script is `Unavailable`.
    pub fn new(script: alloc::vec::Vec<Result<ClassifierSignal, ClassifierError>>) -> Self {
        MockClassifier {
            script,
            next: core::cell::Cell::new(0),
        }
    }
}

impl Classifier for MockClassifier {
    fn classify(&self, _frame: &Frame<'_>) -> Result<ClassifierSignal, ClassifierError> {
        if self.script.is_empty() {
            return Err(ClassifierError::Unavailable);
        }
        let i = self.next.get().min(self.script.len() - 1);
        self.next.set(self.next.get().saturating_add(1));
        self.script[i].clone()
    }
}
