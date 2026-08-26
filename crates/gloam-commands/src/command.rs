use std::{future::Future, pin::Pin};

use crate::{Context, Result};

/// Boxed future returned by generated slash-command adapters.
pub type CommandFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

/// Erased handler function stored in the command registry.
pub type CommandHandler<D> = fn(Context<D>) -> CommandFuture;

/// Static metadata describing a Discord chat-input command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDescriptor {
    /// Command name registered with Discord.
    pub name: &'static str,
    /// Human-readable command description registered with Discord.
    pub description: &'static str,
}

impl CommandDescriptor {
    /// Creates a command descriptor.
    #[must_use]
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self { name, description }
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
    pub const fn new(
        descriptor: &'static CommandDescriptor,
        handler: CommandHandler<D>,
    ) -> Self {
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
