// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};

use super::outcome::{MatchOutcome, Unavailable};
use super::protocol::MatchRequest;

/// A matcher that answers locally, without I/O. Sync by design: the core has no runtime.
pub trait Matcher {
    /// Answers one lookup. Never panics and never returns `Clean` for a failure.
    fn lookup(&self, req: &MatchRequest) -> MatchOutcome;
}

/// Scripted answers, for every downstream test. Not behind a test `cfg`, so consumers can use it.
///
/// Answers are returned in order; after the script is exhausted the last one repeats. An empty
/// script answers `Unavailable(NotConfigured)`, never `Clean`.
#[derive(Debug, Default)]
pub struct MockMatcher {
    script: Vec<MatchOutcome>,
    next: Cell<usize>,
    seen: RefCell<Vec<MatchRequest>>,
}

impl MockMatcher {
    /// A mock that answers with `script` in order.
    pub fn new(script: Vec<MatchOutcome>) -> Self {
        MockMatcher {
            script,
            next: Cell::new(0),
            seen: RefCell::new(Vec::new()),
        }
    }

    /// A mock that always answers `outcome`.
    ///
    /// A deliberate public item: the one-line form for a downstream test that needs a fixed answer
    /// and no script. Exercised in `tests/matcher.rs`.
    pub fn always(outcome: MatchOutcome) -> Self {
        Self::new(alloc::vec![outcome])
    }

    /// Requests received so far, in order.
    pub fn requests(&self) -> Vec<MatchRequest> {
        self.seen.borrow().clone()
    }
}

impl Matcher for MockMatcher {
    fn lookup(&self, req: &MatchRequest) -> MatchOutcome {
        self.seen.borrow_mut().push(req.clone());
        if self.script.is_empty() {
            return MatchOutcome::Unavailable(Unavailable::NotConfigured);
        }
        let i = self.next.get().min(self.script.len() - 1);
        self.next.set(self.next.get().saturating_add(1));
        self.script[i].clone()
    }
}
