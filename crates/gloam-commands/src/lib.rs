//! Slash-command framework built on top of Gloamwire.
//!
//! The framework intentionally supports Discord chat-input application commands
//! only. Prefix/message command parsing is outside the project's scope.

mod command;
mod context;
mod error;
mod framework;
mod registry;
mod runtime;

pub use command::{CommandDescriptor, CommandFuture, CommandHandler, SlashCommand};
pub use context::Context;
pub use error::{Error, Result};
pub use framework::{Framework, FrameworkBuilder};
pub use registry::CommandRegistry;
pub use runtime::Runtime;

/// Common imports for applications using the framework.
pub mod prelude {
    pub use crate::{Context, Framework, Result};
}
