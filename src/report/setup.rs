// SPDX-License-Identifier: MIT OR Apache-2.0
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use super::destination::{DestinationId, ReportDestination, SlotKey};
use super::secret::Credentials;

/// One thing to ask the operator for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// Which destination, by the operator's label.
    pub destination: DestinationId,
    /// Which slot.
    pub key: SlotKey,
    /// The text to show. Generic unless the loaded profile supplied a label.
    pub text: String,
    /// Whether the destination cannot be used without it.
    pub required: bool,
}

/// What setup should tell the operator. Three distinct states: an unconfigured crate must not
/// look like a ready one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupState {
    /// No destination profile is loaded: nothing can be reported, and nothing is asked for.
    NoDestinationsConfigured,
    /// Required credentials are missing.
    Prompts(Vec<Prompt>),
    /// Every required credential of every loaded destination is present.
    Ready,
}

fn prompt_text(id: &DestinationId, key: &SlotKey, label: Option<&str>) -> String {
    match label {
        Some(l) if !l.trim().is_empty() => {
            format!(
                "Credentials for reporting destination `{}`: {l}",
                id.as_str()
            )
        }
        _ => format!(
            "Credentials for reporting destination `{}`, slot `{}` (your registration documents say what to enter)",
            id.as_str(),
            key.as_str()
        ),
    }
}

/// Required slots of `dest` with no non-blank value in `credentials`, as prompts.
pub fn missing_prompts(dest: &dyn ReportDestination, credentials: &Credentials) -> Vec<Prompt> {
    dest.credential_slots()
        .iter()
        .filter(|s| s.required && !credentials.has(&s.key))
        .map(|s| Prompt {
            destination: dest.id().clone(),
            key: s.key.clone(),
            text: prompt_text(dest.id(), &s.key, s.label.as_deref()),
            required: s.required,
        })
        .collect()
}

/// True when every required slot of `dest` is filled.
pub fn credentials_ready(dest: &dyn ReportDestination, credentials: &Credentials) -> bool {
    missing_prompts(dest, credentials).is_empty()
}

/// The setup state across all loaded destinations.
pub fn setup_state(
    destinations: &[&dyn ReportDestination],
    credentials: &Credentials,
) -> SetupState {
    if destinations.is_empty() {
        return SetupState::NoDestinationsConfigured;
    }
    let prompts: Vec<Prompt> = destinations
        .iter()
        .flat_map(|d| missing_prompts(*d, credentials))
        .collect();
    if prompts.is_empty() {
        SetupState::Ready
    } else {
        SetupState::Prompts(prompts)
    }
}
