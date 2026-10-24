// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pre-model screening: the one place a known-hash answer and a classifier signal meet.
//!
//! An application that feeds user images to a model (an inference server, an engine) screens each
//! one first. The decision is a pure function of values already obtained:
//!
//! 1. [`match_request`] hashes the frame (all eight orientations) and refuses a frame whose hash
//!    is too unreliable to match;
//! 2. the host performs the provider exchange and gets a `MatchOutcome`;
//! 3. optionally the host runs a [`Classifier`](crate::classifier::Classifier);
//! 4. [`screen`] decides. Only a `Clean` answer allows, and only then is a [`ScreenedFrame`]
//!    produced, the token a model-facing API should require.
//!
//! The match outcome is consumed for the decision and, on a match, handed back to the caller;
//! it is never given to a classifier.

use crate::classifier::ClassifierSignal;
use crate::hash::{Frame, QUALITY_FLOOR_DEFAULT, pdq_dihedral};
use crate::matcher::{KnownMatch, MatchOutcome, MatchRequest, Unavailable};

/// A frame that passed screening. It can only be obtained from [`screen`] returning
/// [`Screening::Allow`]; a model-facing function that takes `&ScreenedFrame` therefore cannot be
/// called with an unscreened frame.
///
/// ```compile_fail,E0451
/// use safeimage::screening::ScreenedFrame;
/// use safeimage::hash::Frame;
/// let rgb = [0u8; 75];
/// let frame = Frame::new(&rgb, 5, 5).unwrap();
/// // The token has no public constructor and private fields.
/// let _t = ScreenedFrame { frame: &frame };
/// ```
#[derive(Debug)]
pub struct ScreenedFrame<'a, 'f> {
    frame: &'a Frame<'f>,
}

impl<'a, 'f> ScreenedFrame<'a, 'f> {
    /// The screened frame.
    pub fn frame(&self) -> &'a Frame<'f> {
        self.frame
    }
}

/// Why a frame is held (not given to the model, not reported as a match).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScreenHold {
    /// No answer could be obtained. Fail closed.
    Unavailable(Unavailable),
    /// The hash is too unreliable to match (quality at or below the floor). The caller decides:
    /// refuse the image, or send it by a path that does not rely on a hash.
    LowQuality {
        /// The frame's quality score.
        quality: u8,
    },
    /// The classifier's score reached the policy's hold level. A reason to review, not a match.
    ClassifierFlag {
        /// The score.
        score: f32,
    },
}

/// The decision for one frame.
#[derive(Debug)]
pub enum Screening<'a, 'f> {
    /// No known match, and no hold: the frame may go to the model. The classifier signal, if one
    /// was supplied, is returned alongside, unmerged.
    Allow {
        /// The token a model-facing API requires.
        frame: ScreenedFrame<'a, 'f>,
        /// The classifier's signal, if the host ran one.
        classifier: Option<ClassifierSignal>,
    },
    /// Held. Not a match; the host decides what to tell the user.
    Hold(ScreenHold),
    /// A known match. The frame must not reach a model; the host quarantines and reports under
    /// its own rules.
    Blocked(KnownMatch),
}

/// What makes a classifier signal hold a frame.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScreenPolicy {
    /// Hold when the score is at or above this. `None` (the default) never holds on a score, only
    /// passes the signal along.
    pub classifier_hold_at: Option<f32>,
    /// Quality at or below which a frame is held for lack of a usable hash. `None` uses
    /// [`QUALITY_FLOOR_DEFAULT`].
    pub quality_floor: Option<u8>,
}

/// Step 1: the request to send to a provider, or the hold if the frame cannot be matched.
///
/// Uses all eight orientations. A frame at or below the quality floor is
/// [`ScreenHold::LowQuality`]: its hash cannot be trusted either way.
pub fn match_request(frame: &Frame<'_>, policy: &ScreenPolicy) -> Result<MatchRequest, ScreenHold> {
    let d = pdq_dihedral(frame);
    let floor = policy.quality_floor.unwrap_or(QUALITY_FLOOR_DEFAULT);
    if d.quality <= floor {
        return Err(ScreenHold::LowQuality { quality: d.quality });
    }
    Ok(MatchRequest::from_dihedral(&d))
}

/// Step 4: the decision. Pure. A match is blocked, any failure is held, and only a clean answer
/// allows. A classifier signal can only add a hold; it can never turn a match or a failure into an
/// allow.
pub fn screen<'a, 'f>(
    frame: &'a Frame<'f>,
    outcome: MatchOutcome,
    classifier: Option<ClassifierSignal>,
    policy: &ScreenPolicy,
) -> Screening<'a, 'f> {
    match outcome {
        MatchOutcome::Match(m) => Screening::Blocked(m),
        MatchOutcome::Unavailable(r) => Screening::Hold(ScreenHold::Unavailable(r)),
        MatchOutcome::Clean(_) => {
            if let (Some(level), Some(sig)) = (policy.classifier_hold_at, classifier.as_ref())
                && sig.score() >= level
            {
                return Screening::Hold(ScreenHold::ClassifierFlag { score: sig.score() });
            }
            Screening::Allow {
                frame: ScreenedFrame { frame },
                classifier,
            }
        }
    }
}
