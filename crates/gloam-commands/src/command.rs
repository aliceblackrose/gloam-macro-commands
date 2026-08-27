use std::{future::Future, pin::Pin};

use gloamwire::model::{ApplicationCommandNumericValue, ApplicationCommandOptionType};

use crate::{Context, Result};

/// Boxed future returned by generated slash-command adapters.
pub type CommandFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

/// Erased handler function stored in the command registry.
pub type CommandHandler<D> = fn(Context<D>) -> CommandFuture;

/// Static metadata describing one Discord chat-input command option.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandOptionDescriptor {
    /// Option name registered with Discord.
    pub name: &'static str,
    /// Human-readable option description registered with Discord.
    pub description: &'static str,
    /// Discord option kind generated from the Rust parameter type.
    pub kind: ApplicationCommandOptionType,
    /// Whether Discord requires this option in an invocation.
    pub required: bool,
    /// Optional minimum numeric value.
    pub min_value: Option<ApplicationCommandNumericValue>,
    /// Optional maximum numeric value.
    pub max_value: Option<ApplicationCommandNumericValue>,
    /// Optional minimum string length.
    pub min_length: Option<u32>,
    /// Optional maximum string length.
    pub max_length: Option<u32>,
}

impl CommandOptionDescriptor {
    /// Creates an unconstrained command-option descriptor.
    #[must_use]
    pub const fn new(
        name: &'static str,
        description: &'static str,
        kind: ApplicationCommandOptionType,
        required: bool,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            required,
            min_value: None,
            max_value: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Sets the minimum numeric value.
    #[must_use]
    pub const fn min_value(mut self, value: ApplicationCommandNumericValue) -> Self {
        self.min_value = Some(value);
        self
    }

    /// Sets the maximum numeric value.
    #[must_use]
    pub const fn max_value(mut self, value: ApplicationCommandNumericValue) -> Self {
        self.max_value = Some(value);
        self
    }

    /// Sets the minimum string length.
    #[must_use]
    pub const fn min_length(mut self, value: u32) -> Self {
        self.min_length = Some(value);
        self
    }

    /// Sets the maximum string length.
    #[must_use]
    pub const fn max_length(mut self, value: u32) -> Self {
        self.max_length = Some(value);
        self
    }
}

/// Static metadata describing a Discord chat-input command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandDescriptor {
    /// Command name registered with Discord.
    pub name: &'static str,
    /// Human-readable command description registered with Discord.
    pub description: &'static str,
    /// Command options generated from the Rust handler signature.
    pub options: &'static [CommandOptionDescriptor],
}

impl CommandDescriptor {
    /// Creates a command descriptor without options.
    #[must_use]
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            options: &[],
        }
    }

    /// Attaches generated command-option descriptors.
    #[must_use]
    pub const fn with_options(mut self, options: &'static [CommandOptionDescriptor]) -> Self {
        self.options = options;
        self
    }
}

/// Registered slash command and its generated handler adapter.
pub struct SlashCommand<D> {
    descriptor: &'static CommandDescriptor,
    handler: CommandHandler<D>,
}

impl<D> SlashCommand<D> {
    /// Creates a slash command from static metadata and an erased handler.
    #[must_use]
    pub const fn new(descriptor: &'static CommandDescriptor, handler: CommandHandler<D>) -> Self {
        Self {
            descriptor,
            handler,
        }
    }

    /// Returns the static command descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &'static CommandDescriptor {
        self.descriptor
    }

    /// Returns the erased command handler.
    #[must_use]
    pub const fn handler(&self) -> CommandHandler<D> {
        self.handler
    }
}
