//! SafeImage: known-image matching, evidence handling and report building.
//!
//! Layer 1 ([`hash`]) is implemented: perceptual hashing of a decoded frame. The other layers
//! are described in `docs/DESIGN.md` and arrive per `docs/PLAN-step2.md`.

#![no_std]

extern crate alloc;

pub mod hash;
