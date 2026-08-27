use gloamwire::{
    http::{BulkOverwriteApplicationCommand, CreateApplicationCommand},
    model::{ApplicationCommandOption, GuildId},
};

use crate::CommandDescriptor;

/// Discord application-command synchronization policy.
///
/// Registration is opt-in so existing applications that manage Discord command
/// schemas elsewhere preserve their current behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Registration {
    /// Do not modify Discord application commands.
    #[default]
    None,
    /// Replace the application's global command set with the local registry.
    Global,
    /// Replace the application's command set in one guild with the local registry.
    Guild(GuildId),
}

pub(crate) fn create_application_command(
    descriptor: &CommandDescriptor,
) -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::chat_input(descriptor.name, descriptor.description);
    command.options = descriptor
        .options
        .iter()
        .map(application_command_option)
        .collect();
    command
}

pub(crate) fn bulk_guild_command(
    descriptor: &CommandDescriptor,
) -> BulkOverwriteApplicationCommand {
    create_application_command(descriptor).into()
}

fn application_command_option(descriptor: &crate::CommandOptionDescriptor) -> ApplicationCommandOption {
    ApplicationCommandOption {
        kind: descriptor.kind,
        name: descriptor.name.to_owned(),
        name_localizations: None,
        description: descriptor.description.to_owned(),
        description_localizations: None,
        required: descriptor.required.then_some(true),
        choices: Vec::new(),
        options: Vec::new(),
        channel_types: Vec::new(),
        min_value: descriptor.min_value,
        max_value: descriptor.max_value,
        min_length: descriptor.min_length,
        max_length: descriptor.max_length,
        autocomplete: None,
        file_types: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use gloamwire::model::{
        ApplicationCommandNumericValue, ApplicationCommandOptionType, GuildId,
    };

    use crate::{CommandDescriptor, CommandOptionDescriptor};

    use super::{Registration, bulk_guild_command, create_application_command};

    static OPTIONS: &[CommandOptionDescriptor] = &[
        CommandOptionDescriptor::new(
            "query",
            "Search text",
            ApplicationCommandOptionType::STRING,
            true,
        )
        .min_length(1)
        .max_length(64),
        CommandOptionDescriptor::new(
            "limit",
            "Maximum results",
            ApplicationCommandOptionType::INTEGER,
            false,
        )
        .min_integer(1)
        .max_integer(25),
    ];
    static SEARCH: CommandDescriptor =
        CommandDescriptor::new("search", "Search for entries").with_options(OPTIONS);

    #[test]
    fn registration_is_opt_in_by_default() {
        assert_eq!(Registration::default(), Registration::None);
        assert_ne!(Registration::Global, Registration::Guild(GuildId::new(1)));
    }

    #[test]
    fn converts_descriptor_to_gloamwire_command() {
        let command = create_application_command(&SEARCH);

        assert_eq!(command.name, "search");
        assert_eq!(command.description.as_deref(), Some("Search for entries"));
        assert_eq!(command.options.len(), 2);
        assert_eq!(command.options[0].name, "query");
        assert_eq!(command.options[0].required, Some(true));
        assert_eq!(command.options[0].min_length, Some(1));
        assert_eq!(command.options[0].max_length, Some(64));
        assert_eq!(command.options[1].required, None);
        assert_eq!(
            command.options[1].min_value,
            Some(ApplicationCommandNumericValue::Integer(1))
        );
        assert_eq!(
            command.options[1].max_value,
            Some(ApplicationCommandNumericValue::Integer(25))
        );
    }

    #[test]
    fn guild_bulk_command_does_not_invent_existing_id() {
        let command = bulk_guild_command(&SEARCH);
        assert_eq!(command.id, None);
        assert_eq!(command.command.name, "search");
    }
}
