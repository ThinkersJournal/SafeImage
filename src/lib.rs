//! SafeImage: known-image matching, evidence handling and report building.
//!
//! Layers 1 ([`hash`]), 2 ([`matcher`]), 4 ([`report`]) and 5 ([`classifier`], with the [`screening`]
//! join point) are implemented. The evidence layer is described in `docs/DESIGN.md` and arrives per
//! `docs/PLAN-step2.md`.

#![no_std]

extern crate alloc;

pub mod classifier;
pub mod hash;
pub mod matcher;
pub mod report;
pub mod screening;
