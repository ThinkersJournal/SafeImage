use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::input::ReportInput;
use super::state::{ReportState, Step};
use crate::matcher::{HttpRequestSpec, HttpResponseSpec, ProtocolError};

/// The operator's own label for a destination. Opaque to the crate; the crate never derives a
/// name from it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DestinationId(String);

/// Why a [`DestinationId`] is not acceptable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IdError {
    /// Empty.
    Empty,
    /// Longer than 64 bytes.
    TooLong,
    /// Contains a character outside `a-z 0-9 _ -`.
    BadCharacter,
}

fn valid_label(s: &str) -> Result<(), IdError> {
    if s.is_empty() {
        return Err(IdError::Empty);
    }
    if s.len() > 64 {
        return Err(IdError::TooLong);
    }
    if !s
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return Err(IdError::BadCharacter);
    }
    Ok(())
}

impl DestinationId {
    /// Validates and wraps a label.
    pub fn new(label: impl Into<String>) -> Result<Self, IdError> {
        let label = label.into();
        valid_label(&label)?;
        Ok(DestinationId(label))
    }

    /// The label.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The key of a credential slot, as the profile names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotKey(String);

impl SlotKey {
    /// Validates and wraps a key (same character rule as [`DestinationId`]).
    pub fn new(key: impl Into<String>) -> Result<Self, IdError> {
        let key = key.into();
        valid_label(&key)?;
        Ok(SlotKey(key))
    }

    /// The key.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a slot holds, so a setup screen can mask or validate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SlotKind {
    /// A secret (password, token): mask it.
    Secret,
    /// Plain text (a username, a contact name).
    Text,
    /// A URL.
    Url,
}

/// A place an operator must put something a destination needs. Public information only: it
/// never holds a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialSlot {
    /// Key, as the profile names it.
    pub key: SlotKey,
    /// What it holds.
    pub kind: SlotKind,
    /// Whether the destination cannot be used without it.
    pub required: bool,
    /// Human-readable label. **Only a profile supplies one**, so a specific name appears only
    /// where the operator already holds the profile; absent, the setup text is generic.
    pub label: Option<String>,
}

/// The exact document to send, handed back so the host can preserve the bytes it sent.
#[derive(Clone, PartialEq, Eq)]
pub struct BuiltReport {
    /// Document bytes.
    pub bytes: Vec<u8>,
    /// Media type for the request.
    pub content_type: String,
}

impl fmt::Debug for BuiltReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The document holds personal data: print the size only.
        write!(f, "BuiltReport {{ bytes: {} }}", self.bytes.len())
    }
}

/// Why a report could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// A required field is empty. The name is the generic field, never a value.
    MissingField(&'static str),
    /// There are no files to report.
    NoFiles,
    /// Anything else the profile rejects, as text for logs (never contains personal data).
    Invalid(String),
}

/// What a destination said about the step in flight. A closed set every profile maps into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DestinationAnswer {
    /// The step succeeded. `report_id` is set when the answer carries the destination's id for
    /// the report (the submit step).
    Accepted {
        /// The destination's identifier for the report, if the answer carried one.
        report_id: Option<String>,
    },
    /// A network error, a server error or a "try later". Retry with backoff.
    Transient,
    /// The credentials were refused or are not authorised. Raise at once.
    AuthRefused,
    /// The destination rejected the document; no retry can fix it.
    Invalid(String),
    /// The report was already finished (an earlier attempt finished it and its answer was lost).
    AlreadyFinished,
    /// The destination discarded the unfinished report (its deletion window passed).
    WindowExpired,
    /// A code this profile does not map. **Never treated as success**; handled as transient.
    Unknown(String),
}

/// What a profile supplies. Sans-I/O: the host performs the exchange and persists the state
/// between steps.
pub trait ReportDestination {
    /// The operator's label for this destination.
    fn id(&self) -> &DestinationId;

    /// The slots this destination needs, from the loaded profile.
    fn credential_slots(&self) -> &[CredentialSlot];

    /// Builds the document. Credentials never reach this function: the host's transport applies
    /// them, so a built report and a request spec can be logged without leaking a secret.
    fn build(&self, input: &ReportInput) -> Result<BuiltReport, BuildError>;

    /// The request for `step`. `report` is the persisted state (for the destination's id and the
    /// file index in the step); `built` is the document from [`build`](Self::build).
    fn request(
        &self,
        step: &Step,
        report: &ReportState,
        built: &BuiltReport,
    ) -> Result<HttpRequestSpec, ProtocolError>;

    /// Maps the response to `step` to an answer. Never panics; an answer the profile does not
    /// recognise is [`DestinationAnswer::Unknown`], not `Accepted`.
    fn interpret(&self, step: &Step, resp: &HttpResponseSpec) -> DestinationAnswer;
}
