//! Slash-command framework built on top of Gloamwire.
//!
//! The framework intentionally supports Discord chat-input application commands
//! only. Prefix/message command parsing is outside the project's scope.

extern crate self as gloam_commands;

mod command;
mod context;
mod dispatch;
mod error;
mod framework;
mod options;
mod registration;
mod registry;
mod response;
mod runtime;

pub use command::{
    CommandDescriptor, CommandFuture, CommandHandler, CommandOptionDescriptor, SlashCommand,
};
pub use context::Context;
pub use dispatch::{CommandTask, DispatchOutcome};
pub use error::{Error, Result};
pub use framework::{DEFAULT_MAX_CONCURRENT_COMMANDS, Framework, FrameworkBuilder};
pub use gloam_commands_macros::{command, commands, group};
pub use options::{CommandOption, CommandOptions};
pub use registration::Registration;
pub use registry::CommandRegistry;
pub use runtime::Runtime;

/// Common imports for applications using the framework.
pub mod prelude {
    pub use crate::{
        Context, DispatchOutcome, Framework, Registration, Result, command, commands, group,
    };
}
