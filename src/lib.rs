//! SafeImage: known-image matching, evidence handling and report building.
//!
//! All five layers are implemented: [`hash`] (1), [`matcher`] (2), [`evidence`] (3), [`report`] (4)
//! and [`classifier`] (5, with the [`screening`] join point). See `docs/DESIGN.md` and
//! `docs/PLAN-step2.md`.

#![no_std]

extern crate alloc;

pub mod classifier;
pub mod evidence;
pub mod hash;
pub mod matcher;
pub mod report;
pub mod screening;
