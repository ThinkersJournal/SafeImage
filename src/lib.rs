//! SafeImage: known-image matching, evidence handling and report building.
//!
//! Layers 1 ([`hash`]) and 2 ([`matcher`], types, mock and conformance suite) are implemented. The other layers
//! are described in `docs/DESIGN.md` and arrive per `docs/PLAN-step2.md`.

#![no_std]

extern crate alloc;

pub mod hash;
pub mod matcher;
pub mod report;
