// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;

use super::mock::Matcher;
use super::outcome::{
    Classification, CleanBasis, KnownMatch, MatchDetail, MatchOutcome, MatchType, Unavailable,
    strongest,
};
use super::protocol::MatchRequest;
use crate::hash::{Hash256, MATCH_DISTANCE_DEFAULT};

/// One entry of a locally held hash list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    /// The listed hash.
    pub hash: Hash256,
    /// What the list says the item is.
    pub classification: Classification,
    /// Detail kept for the case file.
    pub detail: MatchDetail,
}

/// Where a [`ListMatcher`] reads its list from. The crate contains no list and no sync client:
/// the *operator* syncs a list from a source they are credentialed with and exposes it here.
pub trait ListSource {
    /// The list's version as its source states it, kept in [`CleanBasis`]; `None` if unversioned.
    fn version(&self) -> Option<String>;
    /// The current entries, or `None` when no list has been synced yet.
    fn entries(&self) -> Option<&[ListEntry]>;
}

/// Local Hamming search over an operator-synced list.
///
/// Fail closed: a source with no list, or an empty one, answers
/// `Unavailable(NotConfigured)`, never `Clean`, because an empty list is what a failed sync looks
/// like. `Clean` carries the list version and, as with every answer here, is "no known match",
/// not clearance.
#[derive(Debug)]
pub struct ListMatcher<S: ListSource> {
    source: S,
    threshold: u32,
}

impl<S: ListSource> ListMatcher<S> {
    /// A matcher over `source` using [`MATCH_DISTANCE_DEFAULT`].
    pub fn new(source: S) -> Self {
        Self::with_threshold(source, MATCH_DISTANCE_DEFAULT)
    }

    /// A matcher that treats a Hamming distance at or below `threshold` as a match.
    pub fn with_threshold(source: S, threshold: u32) -> Self {
        ListMatcher { source, threshold }
    }

    /// The source, for the host to refresh it.
    pub fn source(&self) -> &S {
        &self.source
    }
}

impl<S: ListSource> Matcher for ListMatcher<S> {
    fn lookup(&self, req: &MatchRequest) -> MatchOutcome {
        let entries = match self.source.entries() {
            Some(e) if !e.is_empty() => e,
            _ => return MatchOutcome::Unavailable(Unavailable::NotConfigured),
        };
        let mut found: Vec<MatchOutcome> = Vec::new();
        for h in req.hashes() {
            for e in entries {
                let d = h.hamming_distance(&e.hash);
                if d <= self.threshold {
                    let t = if d == 0 {
                        MatchType::Exact
                    } else {
                        MatchType::Near
                    };
                    found.push(MatchOutcome::Match(KnownMatch::new(
                        e.classification.clone(),
                        t,
                        alloc::vec![e.detail.clone()],
                    )));
                }
            }
        }
        if found.is_empty() {
            return MatchOutcome::Clean(CleanBasis {
                list_version: self.source.version(),
                checked_at: None,
            });
        }
        strongest(found)
    }
}
