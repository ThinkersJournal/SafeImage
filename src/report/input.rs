use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// A person named in a report. Debug prints a placeholder so personal data does not reach logs
/// through `{:?}`.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Person {
    /// The operator's identifier for them.
    pub id: String,
    /// Display name or handle.
    pub name: String,
    /// Email address, if held.
    pub email: Option<String>,
    /// Phone number, if held.
    pub phone: Option<String>,
    /// Profile URL, if any.
    pub profile_url: Option<String>,
    /// Postal address, only when every part is present (see [`Address`]).
    pub address: Option<Address>,
}

impl Person {
    /// The address if, and only if, every part is present.
    pub fn reportable_address(&self) -> Option<&Address> {
        self.address.as_ref().filter(|a| a.is_complete())
    }
}

impl fmt::Debug for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Person(<redacted>)")
    }
}

/// A structured postal address. Emitted only when **all** parts are present; a half-filled
/// address is omitted entirely (see [`Person::reportable_address`]), never sent partially, and the
/// report still files.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Address {
    /// Street.
    pub street: String,
    /// City.
    pub city: String,
    /// State or region.
    pub region: String,
    /// Postal code.
    pub postal_code: String,
    /// Country.
    pub country: String,
}

impl Address {
    /// True when every part is non-blank.
    pub fn is_complete(&self) -> bool {
        [
            &self.street,
            &self.city,
            &self.region,
            &self.postal_code,
            &self.country,
        ]
        .iter()
        .all(|p| !p.trim().is_empty())
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Address(<redacted>)")
    }
}

/// One file in a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFile {
    /// Lowercase hex SHA-256 of the file.
    pub sha256_hex: String,
    /// Whether the operator viewed the whole file. Set from evidence of a whole-file view, never
    /// from a button press; `false` for a report queued at match.
    pub viewed_by_operator: bool,
    /// Whether the file was publicly available before it was held.
    pub publicly_available: bool,
}

/// What a report says. Plain data; the destination profile turns it into its own document.
///
/// There is deliberately no IP-address field: an application that stores none leaves the field
/// out, and the type makes the absence explicit instead of defaulting a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportInput {
    /// Who is filing.
    pub reporting_person: Person,
    /// Who the report is about.
    pub reported_person: Person,
    /// Seconds since the Unix epoch of the earliest matched upload.
    pub incident_at: u64,
    /// The files, in the order they will be sent.
    pub files: Vec<ReportFile>,
    /// URLs of the pages that embed the files.
    pub pages: Vec<String>,
}
