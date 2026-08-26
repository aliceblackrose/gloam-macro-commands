use thiserror::Error;

/// Errors produced by the slash-command framework.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A command name was registered more than once.
    #[error("duplicate slash command `{0}`")]
    DuplicateCommand(&'static str),

    /// An error returned by Gloamwire.
    #[error(transparent)]
    Gloamwire(#[from] gloamwire::Error),
}

/// Result type used by the framework.
pub type Result<T = ()> = std::result::Result<T, Error>;
