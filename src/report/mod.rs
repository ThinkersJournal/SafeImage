//! Layer 4: a generic report-filing mechanism. It names no destination.
//!
//! A destination is a *profile* kept outside this repository (`docs/DESIGN.md` section 3.6): it
//! implements [`ReportDestination`] (build the document, make a request for a step, map an
//! answer). Everything here is the part that is the same for every destination: the persisted
//! state of a report, the step vocabulary, the transition function, backoff, credential slots and
//! generic setup prompts.
//!
//! Sans-I/O. The host performs each step, persists the returned state, and decides *when* to
//! file; this layer only says what is due. Credentials never enter a built report or a request
//! spec; the host's transport applies them by slot key.

mod destination;
mod input;
mod secret;
mod setup;
mod state;

pub use destination::{
    BuildError, BuiltReport, CredentialSlot, DestinationAnswer, DestinationId, IdError,
    ReportDestination, SlotKey, SlotKind,
};
pub use input::{Address, Person, ReportFile, ReportInput};
pub use secret::{Credentials, Secret};
pub use setup::{Prompt, SetupState, credentials_ready, missing_prompts, setup_state};
pub use state::{
    Event, FileProgress, FinishNote, HoldReason, ReportState, RetractError, Status, Step,
    WindowPolicy, apply, backoff_secs, deletion_window_elapsed, next_step, request_retract,
    retry_failed, sync_credentials,
};
