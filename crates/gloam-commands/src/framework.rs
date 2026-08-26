use std::sync::Arc;

use gloamwire::RestClient;

use crate::{CommandRegistry, Result, Runtime, SlashCommand};

/// Configured slash-command framework.
pub struct Framework<D> {
    data: Arc<D>,
    registry: CommandRegistry<D>,
}

impl<D> Framework<D> {
    /// Starts configuring a framework around application state.
    #[must_use]
    pub fn builder(data: D) -> FrameworkBuilder<D> {
        FrameworkBuilder::new(data)
    }

    /// Returns the application state shared by command runtimes.
    #[must_use]
    pub fn data(&self) -> &D {
        &self.data
    }

    /// Returns the slash-command registry.
    #[must_use]
    pub const fn registry(&self) -> &CommandRegistry<D> {
        &self.registry
    }

    /// Creates a runtime using this framework's shared application state.
    ///
    /// Phase 3 will make runtime creation part of the managed Gateway execution
    /// path. This method keeps the Phase 1 ownership model explicit and testable.
    #[must_use]
    pub fn runtime(&self, rest: RestClient) -> Runtime<D> {
        Runtime::from_shared(Arc::new(rest), Arc::clone(&self.data))
    }
}

/// Builder for a [`Framework`].
pub struct FrameworkBuilder<D> {
    data: D,
    commands: Vec<SlashCommand<D>>,
}

impl<D> FrameworkBuilder<D> {
    /// Creates a framework builder from application state.
    #[must_use]
    pub const fn new(data: D) -> Self {
        Self {
            data,
            commands: Vec::new(),
        }
    }

    /// Adds one slash command.
    #[must_use]
    pub fn command(mut self, command: SlashCommand<D>) -> Self {
        self.commands.push(command);
        self
    }

    /// Adds multiple slash commands.
    #[must_use]
    pub fn commands(mut self, commands: impl IntoIterator<Item = SlashCommand<D>>) -> Self {
        self.commands.extend(commands);
        self
    }

    /// Validates command registration and builds the framework.
    pub fn build(self) -> Result<Framework<D>> {
        let mut registry = CommandRegistry::new();
        for command in self.commands {
            registry.insert(command)?;
        }

        Ok(Framework {
            data: Arc::new(self.data),
            registry,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{CommandDescriptor, Context, Error, Framework, Result, SlashCommand};

    static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check bot responsiveness");

    fn handler(_ctx: Context<()>) -> crate::CommandFuture {
        Box::pin(async { Ok(()) })
    }

    #[test]
    fn builder_registers_commands() -> Result<()> {
        let framework = Framework::builder(())
            .command(SlashCommand::new(&PING, handler))
            .build()?;

        assert_eq!(framework.registry().len(), 1);
        assert!(framework.registry().get("ping").is_some());
        Ok(())
    }

    #[test]
    fn builder_rejects_duplicate_names() {
        let result = Framework::builder(())
            .command(SlashCommand::new(&PING, handler))
            .command(SlashCommand::new(&PING, handler))
            .build();

        assert!(matches!(result, Err(Error::DuplicateCommand("ping"))));
    }
}
