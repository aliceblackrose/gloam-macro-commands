use thiserror::Error;

/// Errors produced by the slash-command framework.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A command name was registered more than once.
    #[error("duplicate slash command `{0}`")]
    DuplicateCommand(&'static str),

    /// The configured global command-concurrency limit was zero.
    #[error("max concurrent commands must be greater than zero")]
    InvalidConcurrencyLimit,

    /// An `INTERACTION_CREATE` payload could not be decoded.
    #[error("invalid Discord interaction payload: {0}")]
    InvalidInteractionPayload(#[from] serde_json::Error),

    /// An application-command interaction did not include command data.
    #[error("application-command interaction is missing command data")]
    MissingApplicationCommandData,

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
