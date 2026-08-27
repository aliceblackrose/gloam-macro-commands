//! Slash-command framework built on top of Gloamwire.
//!
//! The framework intentionally supports Discord chat-input application commands
//! only. Prefix/message command parsing is outside the project's scope.

extern crate self as gloam_commands;

mod autocomplete;
mod command;
mod context;
mod dispatch;
mod error;
mod framework;
mod options;
mod policy;
mod registration;
mod registry;
mod response;
mod runtime;

pub use autocomplete::{
    AutocompleteChoice, AutocompleteChoiceValue, AutocompleteContext, AutocompleteFuture,
    AutocompleteHandler, AutocompleteHandlerDescriptor,
};
pub use command::{
    CommandChoiceDescriptor, CommandChoiceValue, CommandDescriptor, CommandFuture, CommandHandler,
    CommandOptionDescriptor, SlashCommand,
};
pub use context::Context;
pub use dispatch::{CommandTask, DispatchOutcome};
pub use error::{Error, Result};
pub use framework::{DEFAULT_MAX_CONCURRENT_COMMANDS, Framework, FrameworkBuilder};
pub use gloam_commands_macros::{CommandChoice, autocomplete, check, command, commands, group};
pub use options::{CommandChoice, CommandOption, CommandOptions};
pub use policy::{CheckDescriptor, CheckFuture, CheckHandler, CommandPolicy};
pub use registration::Registration;
pub use registry::CommandRegistry;
pub use runtime::Runtime;

/// Implementation details referenced by generated macro code.
#[doc(hidden)]
pub mod __private {
    pub use gloamwire::model::{ApplicationCommandOptionType, InteractionContextType, Permissions};
}

/// Common imports for applications using the framework.
pub mod prelude {
    pub use crate::{
        AutocompleteChoice, AutocompleteContext, CheckDescriptor, CommandChoice, CommandPolicy,
        Context, DispatchOutcome, Framework, Registration, Result, autocomplete, check, command,
        commands, group,
    };
}
