use std::time::Duration;

use gloamwire::model::Permissions;
use thiserror::Error;

/// Errors produced by the slash-command framework.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A command name was registered more than once.
    #[error("duplicate slash command `{0}`")]
    DuplicateCommand(&'static str),

    /// A nested command path was registered more than once.
    #[error("duplicate slash-command path `{0}`")]
    DuplicateCommandPath(String),

    /// A command tree exceeded Discord's native hierarchy or mixed invalid node types.
    #[error("invalid slash-command hierarchy at `{0}`")]
    InvalidCommandHierarchy(String),

    /// A received interaction did not resolve to a registered command path.
    #[error("unknown slash-command path `{0}`")]
    UnknownCommandPath(String),

    /// Static autocomplete metadata and handler registration were inconsistent.
    #[error("invalid autocomplete configuration at `{0}`")]
    InvalidAutocompleteConfiguration(String),

    /// An autocomplete interaction did not contain exactly one valid focused option.
    #[error("invalid autocomplete focus at `{0}`")]
    InvalidAutocompleteFocus(String),

    /// A focused autocomplete option did not have a registered handler.
    #[error("unknown autocomplete option `{option}` at `{path}`")]
    UnknownAutocompleteOption {
        /// Resolved command path.
        path: String,
        /// Focused option name.
        option: String,
    },

    /// An autocomplete handler returned choices Discord cannot accept.
    #[error("invalid autocomplete response: {0}")]
    InvalidAutocompleteResponse(String),

    /// Static command execution-policy configuration was invalid.
    #[error("invalid command execution policy at `{0}`")]
    InvalidCommandPolicy(String),

    /// The interaction context did not satisfy this command's execution policy.
    #[error("slash command `{0}` is not available in this interaction context")]
    CommandContextNotAllowed(String),

    /// The invoking member did not have every permission required by the command.
    #[error(
        "slash command `{path}` requires member permissions {required:?}, but the interaction supplied {actual:?}"
    )]
    MissingMemberPermissions {
        /// Resolved command path.
        path: String,
        /// Required member permissions.
        required: Permissions,
        /// Permissions supplied by Discord for the invoking member.
        actual: Permissions,
    },

    /// The application did not have every permission required by the command.
    #[error(
        "slash command `{path}` requires application permissions {required:?}, but the interaction supplied {actual:?}"
    )]
    MissingBotPermissions {
        /// Resolved command path.
        path: String,
        /// Required application permissions.
        required: Permissions,
        /// Application permissions supplied by Discord for the interaction channel.
        actual: Permissions,
    },

    /// A custom command check denied execution.
    #[error("slash command `{path}` failed custom check `{check}`")]
    CommandCheckFailed {
        /// Resolved command path.
        path: String,
        /// Generated custom-check name.
        check: &'static str,
    },

    /// A command cooldown could not identify the invoking user.
    #[error(
        "slash command `{0}` cannot apply its cooldown because the invoking user is unavailable"
    )]
    CommandUserUnavailable(String),

    /// A per-user command cooldown is still active.
    #[error("slash command `{path}` is on cooldown for another {retry_after:?}")]
    CommandOnCooldown {
        /// Resolved command path.
        path: String,
        /// Remaining cooldown duration.
        retry_after: Duration,
    },

    /// The configured global command-concurrency limit was zero.
    #[error("max concurrent commands must be greater than zero")]
    InvalidConcurrencyLimit,

    /// An `INTERACTION_CREATE` payload could not be decoded.
    #[error("invalid Discord interaction payload: {0}")]
    InvalidInteractionPayload(#[from] serde_json::Error),

    /// An application-command interaction did not include command data.
    #[error("application-command interaction is missing command data")]
    MissingApplicationCommandData,

    /// A required slash-command option was not submitted.
    #[error("missing required slash-command option `{0}`")]
    MissingOption(&'static str),

    /// A submitted slash-command option did not match its generated Rust type.
    #[error("slash-command option `{name}` is not a valid `{expected}` value")]
    InvalidOption {
        /// Option name generated from the handler parameter.
        name: &'static str,
        /// Rust type expected by the generated handler adapter.
        expected: &'static str,
    },

    /// A submitted slash-command choice did not match any generated enum variant.
    #[error("slash-command option `{name}` contains an unknown choice value")]
    InvalidChoice {
        /// Option name generated from the handler parameter.
        name: &'static str,
    },

    /// An operation required an acknowledgement, but the interaction is still pending.
    #[error("interaction has not been acknowledged")]
    InteractionNotAcknowledged,

    /// A second initial acknowledgement or deferral was attempted.
    #[error("interaction has already been acknowledged")]
    InteractionAlreadyAcknowledged,

    /// The original interaction response was already deleted.
    #[error("original interaction response has already been deleted")]
    OriginalResponseDeleted,

    /// A deferred original response was asked to change public/ephemeral visibility.
    #[error("interaction response visibility cannot be changed after deferral")]
    ResponseVisibilityMismatch,

    /// Manual dispatch was attempted outside a Tokio runtime.
    #[error("command dispatch requires an active Tokio runtime")]
    NoAsyncRuntime,

    /// The internal command scheduler was closed unexpectedly.
    #[error("command execution scheduler is closed")]
    CommandSchedulerClosed,

    /// A spawned command task failed to complete normally.
    #[error("command task failed: {0}")]
    CommandTask(#[from] tokio::task::JoinError),

    /// An error returned by Gloamwire.
    #[error(transparent)]
    Gloamwire(#[from] gloamwire::Error),
}

/// Result type used by the framework.
pub type Result<T = ()> = std::result::Result<T, Error>;
