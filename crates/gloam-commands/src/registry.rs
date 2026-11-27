use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    sync::LazyLock,
};

use gloamwire::model::{ApplicationCommandNumericValue, ApplicationCommandOptionType};
use regex::Regex;
use tokio::sync::Semaphore;

use crate::{CommandChoiceValue, CommandOptionDescriptor, Error, Result, SlashCommand};

static COMMAND_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$")
        .expect("Discord command name regex must be valid")
});

/// Deterministic registry of slash commands keyed by Discord command name.
pub struct CommandRegistry<D> {
    commands: BTreeMap<&'static str, SlashCommand<D>>,
}

impl<D> CommandRegistry<D> {
    /// Creates a slash-command registry.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: BTreeMap::new(),
        }
    }

    /// Inserts one command after validating its Discord-native hierarchy.
    pub fn insert(&mut self, command: SlashCommand<D>) -> Result<()> {
        validate_hierarchy(&command)?;
        let name = command.descriptor().name;
        match self.commands.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(command);
                Ok(())
            }
            Entry::Occupied(_) => Err(Error::DuplicateCommand(name)),
        }
    }

    /// Resolves one top-level command by registered name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&SlashCommand<D>> {
        self.commands.get(name)
    }

    /// Returns the number of registered top-level commands.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns whether the registry has no commands.
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
    validate_node(command, root)?;
    if command.is_leaf() {
        return validate_leaf(command, root);
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
    if children.len() > 25 {
        return Err(Error::InvalidCommandHierarchy(parent_path.join(" ")));
    }
    let mut names = BTreeSet::new();

    for child in children {
        let name = child.descriptor().name;
        let path = format_path(parent_path, name);
        validate_node(child, &path)?;
        if !names.insert(name) {
            return Err(Error::DuplicateCommandPath(path));
        }

        if child.is_leaf() {
            validate_leaf(child, &path)?;
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

fn validate_leaf<D>(command: &SlashCommand<D>, path: &str) -> Result<()> {
    let policy = command
        .policy()
        .expect("leaf commands always carry an execution policy");
    if policy
        .max_concurrent_executions()
        .is_some_and(|limit| limit == 0 || limit > Semaphore::MAX_PERMITS)
    {
        return Err(Error::InvalidCommandPolicy(path.to_owned()));
    }

    let options = command.descriptor().options;
    let handlers = command.autocomplete_handlers();
    let mut handler_names = BTreeSet::new();

    for handler in handlers {
        let name = handler.option_name();
        if !handler_names.insert(name) {
            return Err(Error::InvalidAutocompleteConfiguration(format!(
                "{path} {name}"
            )));
        }

        let Some(option) = options.iter().find(|option| option.name == name) else {
            return Err(Error::InvalidAutocompleteConfiguration(format!(
                "{path} {name}"
            )));
        };
        if !option.autocomplete {
            return Err(Error::InvalidAutocompleteConfiguration(format!(
                "{path} {name}"
            )));
        }
    }

    for option in options.iter().filter(|option| option.autocomplete) {
        if !matches!(
            option.kind,
            ApplicationCommandOptionType::STRING
                | ApplicationCommandOptionType::INTEGER
                | ApplicationCommandOptionType::NUMBER
        ) || !option.choices.is_empty()
            || !handler_names.contains(option.name)
        {
            return Err(Error::InvalidAutocompleteConfiguration(format!(
                "{path} {}",
                option.name
            )));
        }
    }

    validate_options(options, path)
}

fn validate_node<D>(command: &SlashCommand<D>, path: &str) -> Result<()> {
    let descriptor = command.descriptor();
    if !valid_name(descriptor.name) || !valid_description(descriptor.description) {
        return Err(Error::InvalidCommandDescriptor(path.to_owned()));
    }
    Ok(())
}

fn valid_name(name: &str) -> bool {
    COMMAND_NAME.is_match(name) && name.to_lowercase() == name
}

fn valid_description(description: &str) -> bool {
    (1..=100).contains(&description.chars().count())
}

fn validate_options(options: &[CommandOptionDescriptor], path: &str) -> Result<()> {
    if options.len() > 25 {
        return Err(Error::InvalidCommandDescriptor(path.to_owned()));
    }
    let mut names = BTreeSet::new();
    let mut optional_seen = false;
    for option in options {
        if !valid_name(option.name)
            || !valid_description(option.description)
            || !names.insert(option.name)
            || (option.required && optional_seen)
            || !valid_option(option)
        {
            return Err(Error::InvalidCommandDescriptor(format!(
                "{path} {}",
                option.name
            )));
        }
        optional_seen |= !option.required;
    }
    Ok(())
}

fn valid_option(option: &CommandOptionDescriptor) -> bool {
    use ApplicationCommandOptionType as Kind;
    if !matches!(
        option.kind,
        Kind::STRING
            | Kind::BOOLEAN
            | Kind::INTEGER
            | Kind::NUMBER
            | Kind::USER
            | Kind::CHANNEL
            | Kind::ROLE
            | Kind::MENTIONABLE
            | Kind::ATTACHMENT
    ) {
        return false;
    }
    if option.choices.len() > 25 {
        return false;
    }
    if option.min_length.is_some_and(|n| n > 6000)
        || option.max_length.is_some_and(|n| !(1..=6000).contains(&n))
        || option
            .min_length
            .zip(option.max_length)
            .is_some_and(|(min, max)| min > max)
        || (option.kind != Kind::STRING
            && (option.min_length.is_some() || option.max_length.is_some()))
    {
        return false;
    }

    let mut bounds = [None; 2];
    for (index, value) in [option.min_value, option.max_value].into_iter().enumerate() {
        if let Some(value) = value {
            let number = match (option.kind, value) {
                (Kind::INTEGER, ApplicationCommandNumericValue::Integer(n))
                    if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&n) =>
                {
                    n as f64
                }
                (Kind::NUMBER, ApplicationCommandNumericValue::Number(n))
                    if (-9_007_199_254_740_992.0..=9_007_199_254_740_992.0).contains(&n) =>
                {
                    n
                }
                _ => return false,
            };
            bounds[index] = Some(number);
        }
    }
    if bounds[0].zip(bounds[1]).is_some_and(|(min, max)| min > max) {
        return false;
    }
    let mut choice_names = BTreeSet::new();
    for (index, choice) in option.choices.iter().enumerate() {
        if !valid_description(choice.name)
            || !choice_names.insert(choice.name)
            || option.choices[..index]
                .iter()
                .any(|previous| previous.value == choice.value)
        {
            return false;
        }
        match (option.kind, choice.value) {
            (Kind::STRING, CommandChoiceValue::String(value)) if value.chars().count() <= 100 => {}
            (Kind::INTEGER, CommandChoiceValue::Integer(value))
                if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value) => {}
            (Kind::NUMBER, CommandChoiceValue::Number(value))
                if (-9_007_199_254_740_992.0..=9_007_199_254_740_992.0).contains(&value) => {}
            _ => return false,
        }
    }
    true
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
    use gloamwire::model::ApplicationCommandOptionType;

    use super::CommandRegistry;
    use crate::{
        AutocompleteContext, AutocompleteFuture, AutocompleteHandlerDescriptor,
        CommandChoiceDescriptor, CommandDescriptor, CommandOptionDescriptor, CommandPolicy,
        Context, Error, Result, SlashCommand,
    };

    static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check bot responsiveness");
    static ADMIN: CommandDescriptor = CommandDescriptor::new("admin", "Administration commands");
    static BAN: CommandDescriptor = CommandDescriptor::new("ban", "Ban a member");
    static CONFIG: CommandDescriptor = CommandDescriptor::new("config", "Configuration commands");
    static SET: CommandDescriptor = CommandDescriptor::new("set", "Set configuration");
    static DEEPER: CommandDescriptor = CommandDescriptor::new("deeper", "Too deeply nested");
    static SEARCH_OPTIONS: &[CommandOptionDescriptor] = &[CommandOptionDescriptor::new(
        "query",
        "Search text",
        ApplicationCommandOptionType::STRING,
        true,
    )
    .autocomplete()];
    static SEARCH: CommandDescriptor =
        CommandDescriptor::new("search", "Search values").with_options(SEARCH_OPTIONS);
    static INVALID_CHOICES: &[CommandChoiceDescriptor] =
        &[CommandChoiceDescriptor::string("One", "one")];
    static INVALID_AUTOCOMPLETE_OPTIONS: &[CommandOptionDescriptor] =
        &[CommandOptionDescriptor::new(
            "query",
            "Search text",
            ApplicationCommandOptionType::STRING,
            true,
        )
        .with_choices(INVALID_CHOICES)
        .autocomplete()];
    static INVALID_AUTOCOMPLETE: CommandDescriptor =
        CommandDescriptor::new("invalid", "Invalid autocomplete")
            .with_options(INVALID_AUTOCOMPLETE_OPTIONS);

    fn handler(_ctx: Context<()>) -> crate::CommandFuture {
        Box::pin(async { Ok(()) })
    }

    fn autocomplete_handler(_ctx: AutocompleteContext<()>) -> AutocompleteFuture {
        Box::pin(async { Ok(Vec::new()) })
    }

    #[test]
    fn rejects_invalid_manual_descriptor() {
        static INVALID: CommandDescriptor = CommandDescriptor::new("UPPER", "Invalid");
        let mut registry = CommandRegistry::new();
        assert!(matches!(
            registry.insert(SlashCommand::new(&INVALID, handler)),
            Err(Error::InvalidCommandDescriptor(path)) if path == "UPPER"
        ));
    }

    #[test]
    fn rejects_duplicate_manual_options() {
        static OPTIONS: &[CommandOptionDescriptor] = &[
            CommandOptionDescriptor::new(
                "value",
                "First",
                ApplicationCommandOptionType::STRING,
                true,
            ),
            CommandOptionDescriptor::new(
                "value",
                "Second",
                ApplicationCommandOptionType::STRING,
                true,
            ),
        ];
        static INVALID: CommandDescriptor =
            CommandDescriptor::new("invalid", "Invalid").with_options(OPTIONS);
        let mut registry = CommandRegistry::new();
        assert!(matches!(
            registry.insert(SlashCommand::new(&INVALID, handler)),
            Err(Error::InvalidCommandDescriptor(path)) if path == "invalid value"
        ));
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
    fn accepts_valid_autocomplete_configuration() -> Result<()> {
        let command = SlashCommand::new_with_autocomplete(
            &SEARCH,
            handler,
            vec![AutocompleteHandlerDescriptor::new(
                "query",
                autocomplete_handler,
            )],
        );
        let mut registry = CommandRegistry::new();
        registry.insert(command)?;
        Ok(())
    }

    #[test]
    fn rejects_autocomplete_without_handler() {
        let mut registry = CommandRegistry::new();

        assert!(matches!(
            registry.insert(SlashCommand::new(&SEARCH, handler)),
            Err(Error::InvalidAutocompleteConfiguration(path)) if path == "search query"
        ));
    }

    #[test]
    fn rejects_autocomplete_with_static_choices() {
        let command = SlashCommand::new_with_autocomplete(
            &INVALID_AUTOCOMPLETE,
            handler,
            vec![AutocompleteHandlerDescriptor::new(
                "query",
                autocomplete_handler,
            )],
        );
        let mut registry = CommandRegistry::new();

        assert!(matches!(
            registry.insert(command),
            Err(Error::InvalidAutocompleteConfiguration(path)) if path == "invalid query"
        ));
    }

    #[test]
    fn rejects_zero_per_command_concurrency() {
        let command =
            SlashCommand::new_with_policy(&PING, handler, CommandPolicy::new().max_concurrency(0));
        let mut registry = CommandRegistry::new();

        assert!(matches!(
            registry.insert(command),
            Err(Error::InvalidCommandPolicy(path)) if path == "ping"
        ));
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
