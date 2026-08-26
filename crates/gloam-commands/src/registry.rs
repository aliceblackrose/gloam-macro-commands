use std::collections::BTreeMap;

use crate::{Error, Result, SlashCommand};

/// Deterministic registry of slash commands keyed by Discord command name.
pub struct CommandRegistry<D> {
    commands: BTreeMap<&'static str, SlashCommand<D>>,
}

impl<D> CommandRegistry<D> {
    /// Creates an empty command registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }

    /// Inserts a slash command, rejecting duplicate command names.
    pub fn insert(&mut self, command: SlashCommand<D>) -> Result<()> {
        let name = command.descriptor().name;
        if self.commands.contains_key(name) {
            return Err(Error::DuplicateCommand(name));
        }

        self.commands.insert(name, command);
        Ok(())
    }

    /// Returns a registered command by Discord command name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SlashCommand<D>> {
        self.commands.get(name)
    }

    /// Returns the number of registered commands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns whether no commands are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Iterates over commands in deterministic name order.
    pub fn iter(&self) -> impl Iterator<Item = &SlashCommand<D>> {
        self.commands.values()
    }
}

impl<D> Default for CommandRegistry<D> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use gloamwire::RestClient;

    use super::CommandRegistry;
    use crate::{CommandDescriptor, Context, Result, Runtime, SlashCommand};

    static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check bot responsiveness");

    fn handler(_ctx: Context<()>) -> crate::CommandFuture {
        Box::pin(async { Ok(()) })
    }

    #[test]
    fn rejects_duplicate_command_names() -> Result<()> {
        let mut registry = CommandRegistry::new();
        registry.insert(SlashCommand::new(&PING, handler))?;

        let duplicate = registry.insert(SlashCommand::new(&PING, handler));
        assert!(duplicate.is_err());
        Ok(())
    }

    #[test]
    fn resolves_registered_commands() -> Result<()> {
        let mut registry = CommandRegistry::new();
        registry.insert(SlashCommand::new(&PING, handler))?;

        assert_eq!(
            registry.get("ping").map(|command| command.descriptor()),
            Some(&PING)
        );
        assert!(registry.get("missing").is_none());
        Ok(())
    }

    #[allow(dead_code)]
    fn context_constructor_stays_framework_owned(rest: RestClient) {
        let runtime = Arc::new(Runtime::new(rest, ()));
        let context = Context::new(runtime, "ping");
        assert_eq!(context.command_name(), "ping");
    }
}
