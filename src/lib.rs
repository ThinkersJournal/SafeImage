//! SafeImage: known-image matching, evidence handling and report building.
//!
//! All five layers are implemented: [`hash`] (1), [`matcher`] (2), [`evidence`] (3), [`report`] (4)
//! and [`classifier`] (5, with the [`screening`] join point). See `docs/DESIGN.md` and
//! `docs/PLAN-step2.md`.

//! # Which enums are exhaustive
//!
//! Two kinds of public enum, on purpose.
//!
//! **Safety-relevant enums are exhaustive**, so a host's `match` is checked by the compiler and a
//! new variant is a compile error to fix, never a silently ignored case: [`matcher::MatchOutcome`],
//! [`matcher::Unavailable`], [`report::DestinationAnswer`], [`report::Status`], [`report::Step`],
//! [`evidence::EvidenceState`], [`screening::Screening`] and [`screening::ScreenHold`] among them.
//! Adding a variant to one of these is a **breaking change** (the second number, before 1.0).
//!
//! ```
//! use safeimage::matcher::{MatchOutcome, CleanBasis};
//! let o = MatchOutcome::Clean(CleanBasis::default());
//! // No wildcard arm: this compiles only while the enum is exhaustive.
//! let may_publish = match o {
//!     MatchOutcome::Clean(_) => true,
//!     MatchOutcome::Unavailable(_) => false,
//!     MatchOutcome::Match(_) => false,
//! };
//! assert!(may_publish);
//! ```
//!
//! **Informational enums are `#[non_exhaustive]`** (error enums, hints, notes), so a reason can be
//! added without breaking hosts, who must keep a fallback arm:
//!
//! ```compile_fail,E0004
//! use safeimage::hash::FrameError;
//! fn code(e: FrameError) -> i32 {
//!     match e {
//!         FrameError::ZeroDimension => 1,
//!         FrameError::TooLarge => 2,
//!         FrameError::LengthMismatch { .. } => 3,
//!     }
//! }
//! ```

#![no_std]

extern crate alloc;

pub mod classifier;
pub mod evidence;
pub mod hash;
pub mod matcher;
pub mod report;
pub mod screening;
