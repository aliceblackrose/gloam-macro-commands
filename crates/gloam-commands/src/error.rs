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
