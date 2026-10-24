// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use super::destination::SlotKey;

/// A credential value. Opaque: its `Debug` prints a placeholder, it has no `Display`, no
/// `Serialize` and no `Clone`, and the crate never reads it from the environment or a file
/// (design invariant I5). The host reads it from its own secret store and hands it over.
pub struct Secret(String);

impl Secret {
    /// Wraps a value.
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// The value, for the host's transport to apply. The one way out.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

/// The credentials an operator holds, by slot key. Debug prints the keys only.
#[derive(Default)]
pub struct Credentials {
    values: Vec<(SlotKey, Secret)>,
}

impl Credentials {
    /// An empty set.
    pub fn new() -> Self {
        Credentials::default()
    }

    /// Adds or replaces the value for `key`.
    pub fn with(mut self, key: SlotKey, value: Secret) -> Self {
        self.values.retain(|(k, _)| k != &key);
        self.values.push((key, value));
        self
    }

    /// The value for `key`, if supplied and not blank.
    pub fn get(&self, key: &SlotKey) -> Option<&Secret> {
        self.values
            .iter()
            .find(|(k, v)| k == key && !v.expose().trim().is_empty())
            .map(|(_, v)| v)
    }

    /// True if a non-blank value is supplied for `key`.
    pub fn has(&self, key: &SlotKey) -> bool {
        self.get(key).is_some()
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field(
                "slots",
                &self
                    .values
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>(),
            )
            .field("values", &"<redacted>")
            .finish()
    }
}
