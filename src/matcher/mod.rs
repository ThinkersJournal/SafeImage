//! Layer 2: asking "is this hash known?", whoever answers.
//!
//! Sans-I/O: a provider is a pair of pure functions ([`ProviderProtocol`]) and the host performs
//! the exchange. Answers are [`MatchOutcome`]s ordered `Clean` < `Unavailable` < `Match`, so a
//! failure can never be mistaken for a clean image and a match is never lost to a weaker answer.
//!
//! Concrete provider implementations are profiles kept outside this repository
//! (`docs/DESIGN.md` section 3.6). This module has the types, the mock, the retry rule and the
//! conformance suite a profile must pass.

mod conformance;
mod mock;
mod outcome;
mod protocol;

pub use conformance::{Case, Expect, conformance_failures};
pub use mock::{Matcher, MockMatcher};
pub use outcome::{
    Classification, CleanBasis, Digest, KnownMatch, MAX_DETAIL_FIELD_BYTES, MAX_DETAILS,
    MatchDetail, MatchOutcome, MatchType, Unavailable, strongest,
};
pub use protocol::{
    HashKind, HttpMethod, HttpRequestSpec, HttpResponseSpec, MAX_RESPONSE_BYTES, MatchRequest,
    ProtocolError, ProviderProtocol, RequestError, RetryDecision, RetryPolicy, classify_status,
    within_cap,
};
