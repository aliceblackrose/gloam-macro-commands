use std::collections::{BTreeMap, BTreeSet};

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

    /// Inserts a slash command, rejecting duplicate or invalid command paths.
    pub fn insert(&mut self, command: SlashCommand<D>) -> Result<()> {
        validate_hierarchy(&command)?;

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

fn validate_hierarchy<D>(command: &SlashCommand<D>) -> Result<()> {
    let root = command.descriptor().name;
    if command.is_leaf() {
        return Ok(());
    }
    if !command.descriptor().options.is_empty() || command.children().is_empty() {
        return Err(Error::InvalidCommandHierarchy(root.to_owned()));
    }

    validate_children(command.children(), &[root], 1)
}

fn validate_children<D>(
    children: &[SlashCommand<D>],
    parent_path: &[&'static str],
    depth: usize,
) -> Result<()> {
    let mut names = BTreeSet::new();

    for child in children {
        let name = child.descriptor().name;
        let path = format_path(parent_path, name);
        if !names.insert(name) {
            return Err(Error::DuplicateCommandPath(path));
        }

        if child.is_leaf() {
            continue;
        }

        if depth >= 2 || !child.descriptor().options.is_empty() || child.children().is_empty() {
            return Err(Error::InvalidCommandHierarchy(path));
        }
        if child.children().iter().any(SlashCommand::is_group) {
            return Err(Error::InvalidCommandHierarchy(path));
        }

        let mut nested_path = parent_path.to_vec();
        nested_path.push(name);
        validate_children(child.children(), &nested_path, depth + 1)?;
    }

    Ok(())
}

fn format_path(parent: &[&str], child: &str) -> String {
    let mut path = parent.join(" ");
    if !path.is_empty() {
        path.push(' ');
    }
    path.push_str(child);
    path
}

#[cfg(test)]
mod tests {
    use super::CommandRegistry;
    use crate::{CommandDescriptor, Context, Error, Result, SlashCommand};

    static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check bot responsiveness");
    static ADMIN: CommandDescriptor = CommandDescriptor::new("admin", "Administration commands");
    static BAN: CommandDescriptor = CommandDescriptor::new("ban", "Ban a member");
    static CONFIG: CommandDescriptor = CommandDescriptor::new("config", "Configuration commands");
    static SET: CommandDescriptor = CommandDescriptor::new("set", "Set configuration");
    static DEEPER: CommandDescriptor = CommandDescriptor::new("deeper", "Too deeply nested");

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

    #[test]
    fn accepts_discord_native_group_hierarchy() -> Result<()> {
        let command = SlashCommand::group(
            &ADMIN,
            vec![
                SlashCommand::new(&BAN, handler),
                SlashCommand::group(&CONFIG, vec![SlashCommand::new(&SET, handler)]),
            ],
        );
        let mut registry = CommandRegistry::new();
        registry.insert(command)?;
        Ok(())
    }

    #[test]
    fn rejects_duplicate_nested_paths() {
        let command = SlashCommand::group(
            &ADMIN,
            vec![
                SlashCommand::new(&BAN, handler),
                SlashCommand::new(&BAN, handler),
            ],
        );
        let mut registry = CommandRegistry::new();

        assert!(matches!(
            registry.insert(command),
            Err(Error::DuplicateCommandPath(path)) if path == "admin ban"
        ));
    }

    #[test]
    fn rejects_hierarchy_deeper_than_discord_supports() {
        let command = SlashCommand::group(
            &ADMIN,
            vec![SlashCommand::group(
                &CONFIG,
                vec![SlashCommand::group(
                    &DEEPER,
                    vec![SlashCommand::new(&SET, handler)],
                )],
            )],
        );
        let mut registry = CommandRegistry::new();

        assert!(matches!(
            registry.insert(command),
            Err(Error::InvalidCommandHierarchy(path)) if path == "admin config"
        ));
    }
}
