// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use core::fmt;

/// An opaque identity of a person who acts on a case, as the host authenticated them.
///
/// **Known limit:** the crate can only compare these values. One human with two accounts passes as
/// two people; the host must map one human to one id (`docs/DESIGN.md` section 11).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ActorId(String);

impl ActorId {
    /// Wraps a non-blank identity of at most 128 bytes.
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let id = id.into();
        if id.trim().is_empty() || id.len() > 128 {
            None
        } else {
            Some(ActorId(id))
        }
    }

    /// The identity.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ActorId({})", self.0)
    }
}

/// Two different people. The only way to build one is [`TwoPeople::new`], which refuses equal ids,
/// so any operation that takes a `TwoPeople` has had the rule checked by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TwoPeople {
    first: ActorId,
    second: ActorId,
}

/// The two ids were the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamePerson;

impl TwoPeople {
    /// Two different people, or [`SamePerson`].
    pub fn new(first: ActorId, second: ActorId) -> Result<Self, SamePerson> {
        if first == second {
            Err(SamePerson)
        } else {
            Ok(TwoPeople { first, second })
        }
    }

    /// The person who recorded the action.
    pub fn first(&self) -> &ActorId {
        &self.first
    }

    /// The person who approved it.
    pub fn second(&self) -> &ActorId {
        &self.second
    }
}
