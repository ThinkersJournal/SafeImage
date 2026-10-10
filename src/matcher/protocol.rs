use alloc::string::String;
use alloc::vec::Vec;

use super::outcome::{MatchOutcome, Unavailable};
use crate::hash::{Dihedral, Hash256};

/// Default cap on a response body, in bytes. Applied before any parsing.
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// The kind of hash in a request. One kind today; `non_exhaustive` so others can be added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum HashKind {
    /// PDQ, 256 bits.
    Pdq,
}

/// Why a [`MatchRequest`] could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    /// No hashes were given.
    Empty,
    /// More than eight hashes were given.
    TooMany {
        /// Number given.
        given: usize,
    },
}

/// One to eight hashes of the same image (the eight dihedral variants at most).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchRequest {
    kind: HashKind,
    hashes: Vec<Hash256>,
}

impl MatchRequest {
    /// Most hashes in one request.
    pub const MAX_HASHES: usize = 8;

    /// Builds a request of 1 to 8 PDQ hashes.
    pub fn pdq(hashes: Vec<Hash256>) -> Result<Self, RequestError> {
        if hashes.is_empty() {
            return Err(RequestError::Empty);
        }
        if hashes.len() > Self::MAX_HASHES {
            return Err(RequestError::TooMany {
                given: hashes.len(),
            });
        }
        Ok(MatchRequest {
            kind: HashKind::Pdq,
            hashes,
        })
    }

    /// A request carrying all eight dihedral hashes of a frame.
    pub fn from_dihedral(d: &Dihedral) -> Self {
        MatchRequest {
            kind: HashKind::Pdq,
            hashes: d.hashes.to_vec(),
        }
    }

    /// Kind of hash.
    pub fn kind(&self) -> HashKind {
        self.kind
    }

    /// The hashes, in the order given.
    pub fn hashes(&self) -> &[Hash256] {
        &self.hashes
    }
}

/// HTTP method of a [`HttpRequestSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    /// GET.
    Get,
    /// POST.
    Post,
}

/// A request the host must send. Plain data: the crate does no I/O.
///
/// Credentials are **not** in here: the host's transport applies them, so a request spec can be
/// logged without leaking a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequestSpec {
    /// Method.
    pub method: HttpMethod,
    /// Path (and query), relative to the host's configured base URL.
    pub path: String,
    /// Headers, in order.
    pub headers: Vec<(String, String)>,
    /// Body bytes.
    pub body: Vec<u8>,
}

/// A response the host received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponseSpec {
    /// Status code.
    pub status: u16,
    /// Body bytes (the host should not read more than [`MAX_RESPONSE_BYTES`] + 1).
    pub body: Vec<u8>,
}

/// Why a profile could not build a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// The request has a hash kind this profile does not support.
    UnsupportedHashKind,
    /// Any other reason, as text for logs (never contains credentials).
    Other(String),
}

/// A provider as a pair of pure functions. A profile (kept outside this repository) implements
/// this; the host performs the HTTP exchange between the two calls.
pub trait ProviderProtocol {
    /// Builds the request the host must send.
    fn request(&self, req: &MatchRequest) -> Result<HttpRequestSpec, ProtocolError>;

    /// Interprets the response. **Never panics and never returns `Clean` on a failure**: a body
    /// that is over the cap, does not parse to a documented shape, or contradicts itself is
    /// `Unavailable(MalformedResponse)`. Use [`classify_status`] and [`within_cap`] first.
    fn interpret(&self, req: &MatchRequest, resp: &HttpResponseSpec) -> MatchOutcome;
}

/// True when the body is within `cap` bytes. Call before parsing.
pub fn within_cap(body: &[u8], cap: usize) -> bool {
    body.len() <= cap
}

/// Maps a non-success HTTP status to the reason it is unavailable, or `None` for a 2xx status.
///
/// 401 and 403 are credentials refused; 408 and 504 are timeouts; 429 is throttled; any other 5xx
/// is a server error; every other status is unexpected and counts as a malformed response.
/// None of them is ever clean.
pub fn classify_status(status: u16) -> Option<Unavailable> {
    match status {
        200..=299 => None,
        401 | 403 => Some(Unavailable::CredentialsRefused),
        408 | 504 => Some(Unavailable::Timeout),
        429 => Some(Unavailable::Throttled),
        500..=599 => Some(Unavailable::ServerError),
        _ => Some(Unavailable::MalformedResponse),
    }
}

/// What to do after a failed attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryDecision {
    /// Try once more after this many milliseconds.
    Retry {
        /// Delay before the retry.
        after_ms: u64,
    },
    /// Do not retry.
    Stop,
}

/// "One retry inside the caller's deadline", as a pure function: the host owns the clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Retries allowed after the first attempt.
    pub max_retries: u32,
    /// Delay before a retry.
    pub delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        RetryPolicy {
            max_retries: 1,
            delay_ms: 250,
        }
    }
}

impl RetryPolicy {
    /// `attempts_made` is how many attempts have been made so far (1 after the first failure).
    /// Retries only transient reasons (`Timeout`, `ServerError`), never a throttle, a credential
    /// refusal or a malformed answer, and only if the delay still fits before `deadline_ms`.
    pub fn decide(
        &self,
        attempts_made: u32,
        reason: Unavailable,
        elapsed_ms: u64,
        deadline_ms: u64,
    ) -> RetryDecision {
        let transient = matches!(reason, Unavailable::Timeout | Unavailable::ServerError);
        let retries_done = attempts_made.saturating_sub(1);
        if transient
            && retries_done < self.max_retries
            && elapsed_ms.saturating_add(self.delay_ms) < deadline_ms
        {
            RetryDecision::Retry {
                after_ms: self.delay_ms,
            }
        } else {
            RetryDecision::Stop
        }
    }
}
