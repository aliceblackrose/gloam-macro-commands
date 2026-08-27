use gloamwire::{
    RestClient,
    http::{BulkOverwriteApplicationCommand, CreateApplicationCommand},
    model::{
        ApplicationCommand, ApplicationCommandOption, ApplicationId, GuildId,
    },
};

use crate::{CommandDescriptor, CommandOptionDescriptor, CommandRegistry, Result};

/// Discord destination used when synchronizing the local slash-command registry.
///
/// The default is [`Self::None`] so starting a framework never performs a
/// destructive bulk overwrite unless registration is explicitly enabled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Registration {
    /// Replace the application's global command set.
    Global,
    /// Replace the application's command set in one guild.
    Guild(GuildId),
    /// Leave Discord command registration externally managed.
    #[default]
    None,
}

impl Registration {
    pub(crate) async fn synchronize<D>(
        self,
        rest: &RestClient,
        application_id: ApplicationId,
        registry: &CommandRegistry<D>,
    ) -> Result<Vec<ApplicationCommand>> {
        let commands = create_commands(registry);

        match self {
            Self::Global => Ok(rest
                .bulk_overwrite_global_application_commands(application_id, &commands)
                .await?),
            Self::Guild(guild_id) => {
                let commands = commands
                    .into_iter()
                    .map(BulkOverwriteApplicationCommand::from)
                    .collect::<Vec<_>>();
                Ok(rest
                    .bulk_overwrite_guild_application_commands(
                        application_id,
                        guild_id,
                        &commands,
                    )
                    .await?)
            }
            Self::None => Ok(Vec::new()),
        }
    }
}

fn create_commands<D>(registry: &CommandRegistry<D>) -> Vec<CreateApplicationCommand> {
    registry
        .iter()
        .map(|command| create_command(command.descriptor()))
        .collect()
}

fn create_command(descriptor: &CommandDescriptor) -> CreateApplicationCommand {
    let mut command = CreateApplicationCommand::chat_input(descriptor.name, descriptor.description);
    command.options = descriptor.options.iter().map(create_option).collect();
    command
}

fn create_option(descriptor: &CommandOptionDescriptor) -> ApplicationCommandOption {
    ApplicationCommandOption {
        kind: descriptor.kind,
        name: descriptor.name.to_owned(),
        name_localizations: None,
        description: descriptor.description.to_owned(),
        description_localizations: None,
        required: Some(descriptor.required),
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

    use crate::{
        CommandDescriptor, CommandOptionDescriptor, CommandRegistry, Context, Result,
        SlashCommand,
    };

    use super::{Registration, create_commands};

    static QUERY_OPTIONS: &[CommandOptionDescriptor] = &[
        CommandOptionDescriptor::new(
            "count",
            "Number of results",
            ApplicationCommandOptionType::INTEGER,
            true,
        )
        .min_integer(1)
        .max_integer(25),
        CommandOptionDescriptor::new(
            "query",
            "Optional search text",
            ApplicationCommandOptionType::STRING,
            false,
        )
        .min_length(2)
        .max_length(100),
    ];
    static QUERY: CommandDescriptor =
        CommandDescriptor::new("query", "Search for results").with_options(QUERY_OPTIONS);
    static ALPHA: CommandDescriptor = CommandDescriptor::new("alpha", "Alphabetical first");

    fn handler(_ctx: Context<()>) -> crate::CommandFuture {
        Box::pin(async { Ok(()) })
    }

    #[test]
    fn registration_defaults_to_none() {
        assert_eq!(Registration::default(), Registration::None);
        assert_eq!(Registration::Guild(GuildId::new(42)), Registration::Guild(GuildId::new(42)));
    }

    #[test]
    fn converts_descriptors_to_gloamwire_payloads_in_registry_order() -> Result<()> {
        let mut registry = CommandRegistry::new();
        registry.insert(SlashCommand::new(&QUERY, handler))?;
        registry.insert(SlashCommand::new(&ALPHA, handler))?;

        let commands = create_commands(&registry);

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].name, "alpha");
        assert_eq!(commands[1].name, "query");
        assert_eq!(commands[1].description.as_deref(), Some("Search for results"));
        assert_eq!(commands[1].options.len(), 2);

        let count = &commands[1].options[0];
        assert_eq!(count.kind, ApplicationCommandOptionType::INTEGER);
        assert_eq!(count.required, Some(true));
        assert_eq!(count.min_value, Some(ApplicationCommandNumericValue::Integer(1)));
        assert_eq!(count.max_value, Some(ApplicationCommandNumericValue::Integer(25)));

        let query = &commands[1].options[1];
        assert_eq!(query.kind, ApplicationCommandOptionType::STRING);
        assert_eq!(query.required, Some(false));
        assert_eq!(query.min_length, Some(2));
        assert_eq!(query.max_length, Some(100));
        Ok(())
    }
}
