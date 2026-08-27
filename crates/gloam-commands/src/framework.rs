use std::sync::Arc;

use gloamwire::{
    RestClient,
    gateway::{
        GatewayEvent, GatewayIntents, ShardEvent, ShardId, ShardManager, TypedDispatchEvent,
    },
    http::CreateInteractionResponseQuery,
    model::{
        ApplicationCommandChoiceValue, ApplicationCommandInteractionDataOption,
        ApplicationCommandOptionChoice, ApplicationCommandOptionType, ApplicationCommandType,
        ApplicationId, AutocompleteInteractionCallbackData, InteractionCallbackData,
        InteractionCallbackType, InteractionResponse, InteractionType,
    },
};
use tokio::{
    runtime::Handle,
    sync::{Semaphore, TryAcquireError},
    task::JoinSet,
};

use crate::{
    AutocompleteChoice, AutocompleteChoiceValue, AutocompleteContext, AutocompleteHandler,
    CommandFuture, CommandHandler, CommandPolicy, CommandRegistry, CommandTask, Context,
    DispatchOutcome, Error, Registration, Result, Runtime, SlashCommand,
};

/// Default upper bound for simultaneously executing command handlers.
pub const DEFAULT_MAX_CONCURRENT_COMMANDS: usize = 64;

const MAX_AUTOCOMPLETE_CHOICES: usize = 25;
const MAX_AUTOCOMPLETE_NAME_LENGTH: usize = 100;
const MAX_AUTOCOMPLETE_STRING_LENGTH: usize = 100;
const MIN_AUTOCOMPLETE_INTEGER_VALUE: i64 = -9_007_199_254_740_991;
const MAX_AUTOCOMPLETE_INTEGER_VALUE: i64 = 9_007_199_254_740_991;
const MIN_AUTOCOMPLETE_NUMBER_VALUE: f64 = -9_007_199_254_740_992.0;
const MAX_AUTOCOMPLETE_NUMBER_VALUE: f64 = 9_007_199_254_740_992.0;

/// Configured slash-command framework.
pub struct Framework<D> {
    data: Arc<D>,
    registry: CommandRegistry<D>,
    registration: Registration,
    max_concurrent_commands: usize,
    command_slots: Arc<Semaphore>,
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

    /// Returns the configured Discord command-registration target.
    #[must_use]
    pub const fn registration(&self) -> Registration {
        self.registration
    }

    /// Returns the global limit for simultaneously executing commands.
    #[must_use]
    pub const fn max_concurrent_commands(&self) -> usize {
        self.max_concurrent_commands
    }

    /// Creates a runtime using this framework's shared application state.
    #[must_use]
    pub fn runtime(&self, rest: RestClient) -> Runtime<D> {
        Runtime::from_shared(Arc::new(rest), Arc::clone(&self.data))
    }

    /// Synchronizes the local command registry with the configured Discord target.
    ///
    /// This is useful for applications that own their Gateway loop. When
    /// [`Registration::None`] is configured, this method performs no HTTP
    /// requests and returns an empty command list.
    pub async fn synchronize_commands(
        &self,
        rest: &RestClient,
        application_id: ApplicationId,
    ) -> Result<Vec<gloamwire::model::ApplicationCommand>> {
        self.registration
            .synchronize(rest, application_id, &self.registry)
            .await
    }
}

impl<D> Framework<D>
where
    D: Send + Sync + 'static,
{
    /// Routes one Gloamwire Gateway event and spawns a matching slash command.
    ///
    /// Applications that own their Gateway loop can call this for each event.
    /// Events unrelated to chat-input application commands are returned as
    /// [`DispatchOutcome::Ignored`]. If every execution slot is occupied, the
    /// command is not spawned and [`DispatchOutcome::AtCapacity`] is returned.
    pub fn dispatch(&self, rest: &RestClient, event: &GatewayEvent) -> Result<DispatchOutcome> {
        self.spawn_prepared(self.prepare_dispatch(rest, event, None)?)
    }

    /// Routes one sharded Gloamwire Gateway event while preserving shard identity.
    pub fn dispatch_shard(&self, rest: &RestClient, event: &ShardEvent) -> Result<DispatchOutcome> {
        self.spawn_prepared(self.prepare_dispatch(rest, &event.event, Some(event.shard_id))?)
    }

    /// Starts Gloamwire's recommended shard set and dispatches slash commands.
    ///
    /// When command registration is enabled, the local registry is synchronized
    /// exactly once using the application ID from the first Discord `READY`
    /// dispatch. The managed runtime requests no Gateway intents because Discord
    /// interactions do not require an intent. Applications that need additional
    /// event streams can own their Gateway loop and use [`Self::dispatch`] or
    /// [`Self::dispatch_shard`].
    pub async fn run(&self, token: impl Into<String>) -> Result<()> {
        let token = token.into();
        let rest = RestClient::new(&token)?;
        let mut shards = ShardManager::start(token, GatewayIntents::empty(), &rest).await?;

        let result = self.drive_shards(&rest, &mut shards).await;
        if let Err(error) = result {
            let _shutdown_result = shards.shutdown().await;
            return Err(error);
        }

        shards.shutdown().await?;
        Ok(())
    }

    fn spawn_prepared(&self, prepared: PreparedDispatch<D>) -> Result<DispatchOutcome> {
        match prepared {
            PreparedDispatch::Ignored => Ok(DispatchOutcome::Ignored),
            PreparedDispatch::Unregistered(name) => Ok(DispatchOutcome::Unregistered { name }),
            PreparedDispatch::Command(command) => {
                let runtime = Handle::try_current().map_err(|_| Error::NoAsyncRuntime)?;
                let command_name = command.command_name;
                let Some(future) = command.try_execute()? else {
                    return Ok(DispatchOutcome::AtCapacity { name: command_name });
                };
                let handle = runtime.spawn(future);
                Ok(DispatchOutcome::Spawned(CommandTask::new(
                    command_name,
                    handle,
                )))
            }
            PreparedDispatch::Autocomplete(autocomplete) => {
                let runtime = Handle::try_current().map_err(|_| Error::NoAsyncRuntime)?;
                let command_name = autocomplete.command_name;
                let Some(future) = autocomplete.try_execute()? else {
                    return Ok(DispatchOutcome::AtCapacity { name: command_name });
                };
                let handle = runtime.spawn(future);
                Ok(DispatchOutcome::Spawned(CommandTask::new(
                    command_name,
                    handle,
                )))
            }
        }
    }

    fn prepare_dispatch(
        &self,
        rest: &RestClient,
        event: &GatewayEvent,
        shard_id: Option<ShardId>,
    ) -> Result<PreparedDispatch<D>> {
        let GatewayEvent::Dispatch(dispatch) = event else {
            return Ok(PreparedDispatch::Ignored);
        };
        if dispatch.name != "INTERACTION_CREATE" {
            return Ok(PreparedDispatch::Ignored);
        }

        let TypedDispatchEvent::InteractionCreate(interaction) = dispatch.typed()? else {
            return Ok(PreparedDispatch::Ignored);
        };
        if interaction.kind != InteractionType::APPLICATION_COMMAND
            && interaction.kind != InteractionType::APPLICATION_COMMAND_AUTOCOMPLETE
        {
            return Ok(PreparedDispatch::Ignored);
        }

        let interaction_kind = interaction.kind;
        let command_data = interaction
            .application_command_data()?
            .ok_or(Error::MissingApplicationCommandData)?;
        if command_data.kind != ApplicationCommandType::CHAT_INPUT {
            return Ok(PreparedDispatch::Ignored);
        }

        let Some(command) = self.registry.get(&command_data.name) else {
            return Ok(PreparedDispatch::Unregistered(command_data.name));
        };
        let resolved = resolve_command(command, &command_data.options)?;
        let descriptor = resolved.command.descriptor();
        let runtime = Arc::new(self.runtime(rest.clone()));
        let interaction = Arc::from(interaction);
        let command_data = Arc::new(command_data);

        if interaction_kind == InteractionType::APPLICATION_COMMAND_AUTOCOMPLETE {
            let focused_index = focused_option_index(&resolved.options, &resolved.path)?;
            let focused = &resolved.options[focused_index];
            let Some(option_descriptor) = descriptor.options.iter().find(|option| {
                option.name == focused.name && option.kind == focused.kind && option.autocomplete
            }) else {
                return Err(Error::UnknownAutocompleteOption {
                    path: resolved.path.join(" "),
                    option: focused.name.clone(),
                });
            };
            let handler = resolved
                .command
                .autocomplete_handler(option_descriptor.name)
                .ok_or_else(|| Error::UnknownAutocompleteOption {
                    path: resolved.path.join(" "),
                    option: focused.name.clone(),
                })?;
            let context = AutocompleteContext::new(
                runtime,
                interaction,
                command_data,
                resolved.path,
                resolved.options,
                focused_index,
                shard_id,
            );

            return Ok(PreparedDispatch::Autocomplete(PreparedAutocomplete {
                command_name: descriptor.name,
                option_kind: option_descriptor.kind,
                handler,
                context,
                command_slots: Arc::clone(&self.command_slots),
            }));
        }

        let handler = resolved
            .command
            .handler()
            .ok_or_else(|| Error::UnknownCommandPath(resolved.path.join(" ")))?;
        let policy = resolved
            .command
            .shared_policy()
            .ok_or_else(|| Error::UnknownCommandPath(resolved.path.join(" ")))?;
        let context = Context::new(
            runtime,
            interaction,
            command_data,
            resolved.path,
            resolved.options,
            shard_id,
        );

        Ok(PreparedDispatch::Command(PreparedCommand {
            command_name: descriptor.name,
            handler,
            policy,
            context,
            command_slots: Arc::clone(&self.command_slots),
        }))
    }

    async fn drive_shards(&self, rest: &RestClient, shards: &mut ShardManager) -> Result<()> {
        let mut commands = JoinSet::new();
        let mut synchronized = self.registration == Registration::None;

        while let Some(event) = shards.next_event().await {
            reap_completed_commands(&mut commands)?;
            let event = event?;

            if !synchronized && let Some(application_id) = ready_application_id(&event.event)? {
                self.synchronize_commands(rest, application_id).await?;
                synchronized = true;
            }

            match self.prepare_dispatch(rest, &event.event, Some(event.shard_id))? {
                PreparedDispatch::Command(command) => {
                    if let Some(future) = command.try_execute()? {
                        commands.spawn(future);
                    }
                }
                PreparedDispatch::Autocomplete(autocomplete) => {
                    if let Some(future) = autocomplete.try_execute()? {
                        commands.spawn(future);
                    }
                }
                PreparedDispatch::Ignored | PreparedDispatch::Unregistered(_) => {}
            }
        }

        reap_completed_commands(&mut commands)?;
        Ok(())
    }
}

/// Builder for a [`Framework`].
pub struct FrameworkBuilder<D> {
    data: D,
    commands: Vec<SlashCommand<D>>,
    registration: Registration,
    max_concurrent_commands: usize,
}

impl<D> FrameworkBuilder<D> {
    /// Creates a framework builder from application state.
    #[must_use]
    pub const fn new(data: D) -> Self {
        Self {
            data,
            commands: Vec::new(),
            registration: Registration::None,
            max_concurrent_commands: DEFAULT_MAX_CONCURRENT_COMMANDS,
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

    /// Configures where the local command registry is synchronized in managed mode.
    #[must_use]
    pub const fn registration(mut self, registration: Registration) -> Self {
        self.registration = registration;
        self
    }

    /// Sets the global maximum number of simultaneously executing commands.
    #[must_use]
    pub const fn max_concurrent_commands(mut self, limit: usize) -> Self {
        self.max_concurrent_commands = limit;
        self
    }

    /// Validates command registration and builds the framework.
    pub fn build(self) -> Result<Framework<D>> {
        if self.max_concurrent_commands == 0 {
            return Err(Error::InvalidConcurrencyLimit);
        }

        let mut registry = CommandRegistry::new();
        for command in self.commands {
            registry.insert(command)?;
        }

        Ok(Framework {
            data: Arc::new(self.data),
            registry,
            registration: self.registration,
            max_concurrent_commands: self.max_concurrent_commands,
            command_slots: Arc::new(Semaphore::new(self.max_concurrent_commands)),
        })
    }
}

struct ResolvedCommand<'a, D> {
    command: &'a SlashCommand<D>,
    path: Vec<&'static str>,
    options: Vec<ApplicationCommandInteractionDataOption>,
}

fn resolve_command<'a, D>(
    command: &'a SlashCommand<D>,
    submitted: &[ApplicationCommandInteractionDataOption],
) -> Result<ResolvedCommand<'a, D>> {
    let mut path = vec![command.descriptor().name];
    if command.is_leaf() {
        return Ok(ResolvedCommand {
            command,
            path,
            options: submitted.to_vec(),
        });
    }

    let branch = only_branch(submitted, &path)?;
    let Some(child) = command
        .children()
        .iter()
        .find(|child| child.descriptor().name == branch.name)
    else {
        return Err(Error::UnknownCommandPath(format!(
            "{} {}",
            path.join(" "),
            branch.name
        )));
    };
    path.push(child.descriptor().name);

    if child.is_leaf() {
        if branch.kind != ApplicationCommandOptionType::SUB_COMMAND {
            return Err(Error::UnknownCommandPath(path.join(" ")));
        }
        return Ok(ResolvedCommand {
            command: child,
            path,
            options: branch.options.clone(),
        });
    }

    if branch.kind != ApplicationCommandOptionType::SUB_COMMAND_GROUP {
        return Err(Error::UnknownCommandPath(path.join(" ")));
    }
    let nested = only_branch(&branch.options, &path)?;
    let Some(leaf) = child
        .children()
        .iter()
        .find(|candidate| candidate.descriptor().name == nested.name)
    else {
        return Err(Error::UnknownCommandPath(format!(
            "{} {}",
            path.join(" "),
            nested.name
        )));
    };
    path.push(leaf.descriptor().name);
    if !leaf.is_leaf() || nested.kind != ApplicationCommandOptionType::SUB_COMMAND {
        return Err(Error::UnknownCommandPath(path.join(" ")));
    }

    Ok(ResolvedCommand {
        command: leaf,
        path,
        options: nested.options.clone(),
    })
}

fn only_branch<'a>(
    submitted: &'a [ApplicationCommandInteractionDataOption],
    path: &[&str],
) -> Result<&'a ApplicationCommandInteractionDataOption> {
    if submitted.len() != 1 {
        return Err(Error::UnknownCommandPath(path.join(" ")));
    }
    Ok(&submitted[0])
}

fn focused_option_index(
    options: &[ApplicationCommandInteractionDataOption],
    path: &[&str],
) -> Result<usize> {
    let mut focused = options
        .iter()
        .enumerate()
        .filter(|(_, option)| option.focused == Some(true));
    let Some((index, _)) = focused.next() else {
        return Err(Error::InvalidAutocompleteFocus(path.join(" ")));
    };
    if focused.next().is_some() {
        return Err(Error::InvalidAutocompleteFocus(path.join(" ")));
    }
    Ok(index)
}

struct PreparedCommand<D> {
    command_name: &'static str,
    handler: CommandHandler<D>,
    policy: Arc<CommandPolicy<D>>,
    context: Context<D>,
    command_slots: Arc<Semaphore>,
}

impl<D> PreparedCommand<D>
where
    D: Send + Sync + 'static,
{
    fn try_execute(self) -> Result<Option<CommandFuture>> {
        let Self {
            handler,
            policy,
            context,
            command_slots,
            ..
        } = self;
        let command_permit = match policy.command_slots() {
            Some(slots) => match Arc::clone(slots).try_acquire_owned() {
                Ok(permit) => Some(permit),
                Err(TryAcquireError::NoPermits) => return Ok(None),
                Err(TryAcquireError::Closed) => return Err(Error::CommandSchedulerClosed),
            },
            None => None,
        };
        let permit = match command_slots.try_acquire_owned() {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => return Ok(None),
            Err(TryAcquireError::Closed) => return Err(Error::CommandSchedulerClosed),
        };

        Ok(Some(Box::pin(async move {
            let _permit = permit;
            let _command_permit = command_permit;
            policy.evaluate(&context).await?;
            (handler)(context).await
        })))
    }
}

struct PreparedAutocomplete<D> {
    command_name: &'static str,
    option_kind: ApplicationCommandOptionType,
    handler: AutocompleteHandler<D>,
    context: AutocompleteContext<D>,
    command_slots: Arc<Semaphore>,
}

impl<D> PreparedAutocomplete<D>
where
    D: Send + Sync + 'static,
{
    fn try_execute(self) -> Result<Option<CommandFuture>> {
        let Self {
            option_kind,
            handler,
            context,
            command_slots,
            ..
        } = self;
        let permit = match command_slots.try_acquire_owned() {
            Ok(permit) => permit,
            Err(TryAcquireError::NoPermits) => return Ok(None),
            Err(TryAcquireError::Closed) => return Err(Error::CommandSchedulerClosed),
        };

        Ok(Some(Box::pin(async move {
            let _permit = permit;
            execute_autocomplete(option_kind, handler, context).await
        })))
    }
}

async fn execute_autocomplete<D>(
    option_kind: ApplicationCommandOptionType,
    handler: AutocompleteHandler<D>,
    context: AutocompleteContext<D>,
) -> Result<()>
where
    D: Send + Sync + 'static,
{
    let rest = context.rest().clone();
    let interaction_id = context.interaction().id;
    let interaction_token = context.interaction().token.clone();
    let choices = (handler)(context).await?;
    let choices = autocomplete_response_choices(option_kind, choices)?;
    let response = InteractionResponse {
        kind: InteractionCallbackType::APPLICATION_COMMAND_AUTOCOMPLETE_RESULT,
        data: Some(InteractionCallbackData::Autocomplete(
            AutocompleteInteractionCallbackData { choices },
        )),
    };

    rest.create_interaction_response(
        interaction_id,
        &interaction_token,
        &response,
        &CreateInteractionResponseQuery::default(),
    )
    .await?;
    Ok(())
}

fn autocomplete_response_choices(
    option_kind: ApplicationCommandOptionType,
    choices: Vec<AutocompleteChoice>,
) -> Result<Vec<ApplicationCommandOptionChoice>> {
    if choices.len() > MAX_AUTOCOMPLETE_CHOICES {
        return Err(Error::InvalidAutocompleteResponse(format!(
            "handlers may return at most {MAX_AUTOCOMPLETE_CHOICES} choices"
        )));
    }

    choices
        .into_iter()
        .map(|choice| autocomplete_response_choice(option_kind, choice))
        .collect()
}

fn autocomplete_response_choice(
    option_kind: ApplicationCommandOptionType,
    choice: AutocompleteChoice,
) -> Result<ApplicationCommandOptionChoice> {
    let name_length = choice.name.chars().count();
    if name_length == 0 || name_length > MAX_AUTOCOMPLETE_NAME_LENGTH {
        return Err(Error::InvalidAutocompleteResponse(format!(
            "choice names must contain 1 to {MAX_AUTOCOMPLETE_NAME_LENGTH} characters"
        )));
    }

    let value = match (option_kind, choice.value) {
        (ApplicationCommandOptionType::STRING, AutocompleteChoiceValue::String(value)) => {
            let value_length = value.chars().count();
            if value_length > MAX_AUTOCOMPLETE_STRING_LENGTH {
                return Err(Error::InvalidAutocompleteResponse(format!(
                    "string choice values must contain at most {MAX_AUTOCOMPLETE_STRING_LENGTH} characters"
                )));
            }
            ApplicationCommandChoiceValue::String(value)
        }
        (ApplicationCommandOptionType::INTEGER, AutocompleteChoiceValue::Integer(value)) => {
            if !(MIN_AUTOCOMPLETE_INTEGER_VALUE..=MAX_AUTOCOMPLETE_INTEGER_VALUE).contains(&value) {
                return Err(Error::InvalidAutocompleteResponse(
                    "integer choice values must be within Discord's safe integer range".to_owned(),
                ));
            }
            ApplicationCommandChoiceValue::Integer(value)
        }
        (ApplicationCommandOptionType::NUMBER, AutocompleteChoiceValue::Number(value)) => {
            if !value.is_finite()
                || !(MIN_AUTOCOMPLETE_NUMBER_VALUE..=MAX_AUTOCOMPLETE_NUMBER_VALUE).contains(&value)
            {
                return Err(Error::InvalidAutocompleteResponse(
                    "number choice values must be finite and within Discord's numeric range"
                        .to_owned(),
                ));
            }
            ApplicationCommandChoiceValue::Number(value)
        }
        _ => {
            return Err(Error::InvalidAutocompleteResponse(
                "choice value type does not match the focused option type".to_owned(),
            ));
        }
    };

    Ok(ApplicationCommandOptionChoice {
        name: choice.name,
        name_localizations: None,
        value,
    })
}

enum PreparedDispatch<D> {
    Ignored,
    Unregistered(String),
    Command(PreparedCommand<D>),
    Autocomplete(PreparedAutocomplete<D>),
}

fn ready_application_id(event: &GatewayEvent) -> Result<Option<ApplicationId>> {
    let GatewayEvent::Dispatch(dispatch) = event else {
        return Ok(None);
    };
    if dispatch.name != "READY" {
        return Ok(None);
    }

    let TypedDispatchEvent::Ready(ready) = dispatch.typed()? else {
        return Ok(None);
    };
    Ok(Some(ready.application.id))
}

fn reap_completed_commands(commands: &mut JoinSet<Result<()>>) -> Result<()> {
    while let Some(result) = commands.try_join_next() {
        match result {
            Ok(Ok(())) | Ok(Err(_)) => {
                // Phase 11 adds configurable centralized command-error handling.
            }
            Err(error) => return Err(Error::CommandTask(error)),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicI64, AtomicU32, AtomicU64, AtomicUsize, Ordering},
    };

    use gloamwire::{
        RestClient,
        gateway::{DispatchEvent, GatewayEvent, ShardEvent, ShardId},
        model::{ApplicationCommandOptionType, ApplicationId, GuildId},
    };
    use tokio::sync::Semaphore;

    use crate::{
        CommandDescriptor, CommandOption, CommandOptionDescriptor, CommandTask, Context,
        DispatchOutcome, Error, Framework, Registration, Result, SlashCommand,
    };

    use super::{DEFAULT_MAX_CONCURRENT_COMMANDS, ready_application_id};

    static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check bot responsiveness");
    static SLOW: CommandDescriptor = CommandDescriptor::new("slow", "Exercise concurrency limits");
    static ADMIN: CommandDescriptor = CommandDescriptor::new("admin", "Administration commands");
    static CONFIG: CommandDescriptor = CommandDescriptor::new("config", "Configuration commands");
    static SET_OPTIONS: &[CommandOptionDescriptor] = &[CommandOptionDescriptor::new(
        "count",
        "Configured value",
        ApplicationCommandOptionType::INTEGER,
        true,
    )];
    static SET: CommandDescriptor =
        CommandDescriptor::new("set", "Set configuration").with_options(SET_OPTIONS);

    fn handler(_ctx: Context<()>) -> crate::CommandFuture {
        Box::pin(async { Ok(()) })
    }

    struct DispatchState {
        calls: AtomicUsize,
        interaction_id: AtomicU64,
        shard_id: AtomicU32,
    }

    impl DispatchState {
        const fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                interaction_id: AtomicU64::new(0),
                shard_id: AtomicU32::new(u32::MAX),
            }
        }
    }

    fn dispatch_handler(ctx: Context<DispatchState>) -> crate::CommandFuture {
        Box::pin(async move {
            ctx.data().calls.fetch_add(1, Ordering::SeqCst);
            ctx.data()
                .interaction_id
                .store(ctx.interaction().id.get(), Ordering::SeqCst);
            ctx.data().shard_id.store(
                ctx.shard_id().map_or(u32::MAX, ShardId::get),
                Ordering::SeqCst,
            );
            Ok(())
        })
    }

    struct NestedDispatchState {
        path: Mutex<Vec<&'static str>>,
        count: AtomicI64,
    }

    impl NestedDispatchState {
        fn new() -> Self {
            Self {
                path: Mutex::new(Vec::new()),
                count: AtomicI64::new(0),
            }
        }
    }

    fn nested_handler(ctx: Context<NestedDispatchState>) -> crate::CommandFuture {
        Box::pin(async move {
            let count = i64::extract(&ctx.command_options(), "count")?;
            *ctx.data().path.lock().expect("path mutex") = ctx.command_path().to_vec();
            ctx.data().count.store(count, Ordering::SeqCst);
            Ok(())
        })
    }

    struct ConcurrencyState {
        active: AtomicUsize,
        max_active: AtomicUsize,
        entered: AtomicUsize,
        release: Semaphore,
    }

    impl ConcurrencyState {
        fn new() -> Self {
            Self {
                active: AtomicUsize::new(0),
                max_active: AtomicUsize::new(0),
                entered: AtomicUsize::new(0),
                release: Semaphore::new(0),
            }
        }
    }

    fn slow_handler(ctx: Context<ConcurrencyState>) -> crate::CommandFuture {
        Box::pin(async move {
            let active = ctx.data().active.fetch_add(1, Ordering::SeqCst) + 1;
            ctx.data().max_active.fetch_max(active, Ordering::SeqCst);
            ctx.data().entered.fetch_add(1, Ordering::SeqCst);

            let permit = ctx
                .data()
                .release
                .acquire()
                .await
                .expect("test release semaphore remains open");
            permit.forget();
            ctx.data().active.fetch_sub(1, Ordering::SeqCst);
            Ok(())
        })
    }

    #[test]
    fn builder_registers_commands() -> Result<()> {
        let framework = Framework::builder(())
            .command(SlashCommand::new(&PING, handler))
            .build()?;

        assert_eq!(framework.registry().len(), 1);
        assert!(framework.registry().get("ping").is_some());
        assert_eq!(framework.registration(), Registration::None);
        assert_eq!(
            framework.max_concurrent_commands(),
            DEFAULT_MAX_CONCURRENT_COMMANDS
        );
        Ok(())
    }

    #[test]
    fn builder_configures_registration_target() -> Result<()> {
        let registration = Registration::Guild(GuildId::new(42));
        let framework = Framework::builder(()).registration(registration).build()?;

        assert_eq!(framework.registration(), registration);
        Ok(())
    }

    #[test]
    fn extracts_application_id_from_ready_dispatch() -> Result<()> {
        let event = GatewayEvent::Dispatch(DispatchEvent {
            name: "READY".to_owned(),
            sequence: 1,
            data: serde_json::json!({
                "v": 10,
                "user": {
                    "id": "100",
                    "username": "bot",
                    "discriminator": "0",
                    "avatar": null
                },
                "guilds": [],
                "session_id": "session",
                "resume_gateway_url": "wss://gateway.discord.gg",
                "application": {
                    "id": "200",
                    "flags": 0
                }
            }),
        });

        assert_eq!(ready_application_id(&event)?, Some(ApplicationId::new(200)));
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

    #[test]
    fn builder_rejects_zero_concurrency() {
        let result = Framework::builder(()).max_concurrent_commands(0).build();
        assert!(matches!(result, Err(Error::InvalidConcurrencyLimit)));
    }

    #[test]
    fn builder_configures_concurrency_limit() -> Result<()> {
        let framework = Framework::builder(()).max_concurrent_commands(3).build()?;
        assert_eq!(framework.max_concurrent_commands(), 3);
        Ok(())
    }

    #[tokio::test]
    async fn ignores_unrelated_gateway_events() -> Result<()> {
        let framework = Framework::builder(()).build()?;
        let rest = RestClient::new("test-token")?;

        assert!(matches!(
            framework.dispatch(&rest, &GatewayEvent::HeartbeatAck)?,
            DispatchOutcome::Ignored
        ));
        Ok(())
    }

    #[tokio::test]
    async fn reports_unregistered_chat_input_commands() -> Result<()> {
        let framework = Framework::builder(()).build()?;
        let rest = RestClient::new("test-token")?;
        let event = interaction_event("missing");

        assert!(matches!(
            framework.dispatch(&rest, &event)?,
            DispatchOutcome::Unregistered { name } if name == "missing"
        ));
        Ok(())
    }

    #[tokio::test]
    async fn dispatches_registered_chat_input_commands() -> Result<()> {
        let framework = Framework::builder(DispatchState::new())
            .command(SlashCommand::new(&PING, dispatch_handler))
            .build()?;
        let rest = RestClient::new("test-token")?;
        let event = interaction_event("ping");

        spawned(framework.dispatch(&rest, &event)?).join().await?;

        assert_eq!(framework.data().calls.load(Ordering::SeqCst), 1);
        assert_eq!(framework.data().interaction_id.load(Ordering::SeqCst), 100);
        assert_eq!(framework.data().shard_id.load(Ordering::SeqCst), u32::MAX);
        Ok(())
    }

    #[tokio::test]
    async fn dispatches_nested_subcommand_with_leaf_option_scope() -> Result<()> {
        let framework = Framework::builder(NestedDispatchState::new())
            .command(SlashCommand::group(
                &ADMIN,
                vec![SlashCommand::group(
                    &CONFIG,
                    vec![SlashCommand::new(&SET, nested_handler)],
                )],
            ))
            .build()?;
        let rest = RestClient::new("test-token")?;
        let event = nested_interaction_event();

        spawned(framework.dispatch(&rest, &event)?).join().await?;

        assert_eq!(
            framework.data().path.lock().expect("path mutex").as_slice(),
            ["admin", "config", "set"]
        );
        assert_eq!(framework.data().count.load(Ordering::SeqCst), 7);
        Ok(())
    }

    #[tokio::test]
    async fn rejects_unknown_nested_command_paths() -> Result<()> {
        let framework = Framework::builder(())
            .command(SlashCommand::group(
                &ADMIN,
                vec![SlashCommand::new(&PING, handler)],
            ))
            .build()?;
        let rest = RestClient::new("test-token")?;
        let event = GatewayEvent::Dispatch(DispatchEvent {
            name: "INTERACTION_CREATE".to_owned(),
            sequence: 1,
            data: serde_json::json!({
                "id": "100",
                "application_id": "200",
                "type": 2,
                "data": {
                    "id": "300",
                    "name": "admin",
                    "type": 1,
                    "options": [{"name":"missing","type":1}]
                },
                "token": "interaction-token",
                "version": 1
            }),
        });

        assert!(matches!(
            framework.dispatch(&rest, &event),
            Err(Error::UnknownCommandPath(path)) if path == "admin missing"
        ));
        Ok(())
    }

    #[tokio::test]
    async fn preserves_shard_identity_during_dispatch() -> Result<()> {
        let framework = Framework::builder(DispatchState::new())
            .command(SlashCommand::new(&PING, dispatch_handler))
            .build()?;
        let rest = RestClient::new("test-token")?;
        let event = ShardEvent {
            shard_id: ShardId::new(7),
            event: interaction_event("ping"),
        };

        spawned(framework.dispatch_shard(&rest, &event)?)
            .join()
            .await?;

        assert_eq!(framework.data().shard_id.load(Ordering::SeqCst), 7);
        Ok(())
    }

    #[tokio::test]
    async fn rejects_commands_at_capacity_without_spawning_waiters() -> Result<()> {
        let framework = Framework::builder(ConcurrencyState::new())
            .command(SlashCommand::new(&SLOW, slow_handler))
            .max_concurrent_commands(1)
            .build()?;
        let rest = RestClient::new("test-token")?;
        let first = spawned(framework.dispatch(&rest, &interaction_event("slow"))?);

        assert!(matches!(
            framework.dispatch(&rest, &interaction_event("slow"))?,
            DispatchOutcome::AtCapacity { name } if name == "slow"
        ));

        while framework.data().entered.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(framework.data().max_active.load(Ordering::SeqCst), 1);

        framework.data().release.add_permits(1);
        first.join().await?;

        let second = spawned(framework.dispatch(&rest, &interaction_event("slow"))?);
        while framework.data().entered.load(Ordering::SeqCst) < 2 {
            tokio::task::yield_now().await;
        }
        framework.data().release.add_permits(1);
        second.join().await?;

        assert_eq!(framework.data().max_active.load(Ordering::SeqCst), 1);
        Ok(())
    }

    fn interaction_event(command_name: &str) -> GatewayEvent {
        GatewayEvent::Dispatch(DispatchEvent {
            name: "INTERACTION_CREATE".to_owned(),
            sequence: 1,
            data: serde_json::json!({
                "id": "100",
                "application_id": "200",
                "type": 2,
                "data": {
                    "id": "300",
                    "name": command_name,
                    "type": 1
                },
                "token": "interaction-token",
                "version": 1
            }),
        })
    }

    fn nested_interaction_event() -> GatewayEvent {
        GatewayEvent::Dispatch(DispatchEvent {
            name: "INTERACTION_CREATE".to_owned(),
            sequence: 1,
            data: serde_json::json!({
                "id": "100",
                "application_id": "200",
                "type": 2,
                "data": {
                    "id": "300",
                    "name": "admin",
                    "type": 1,
                    "options": [{
                        "name": "config",
                        "type": 2,
                        "options": [{
                            "name": "set",
                            "type": 1,
                            "options": [{"name":"count","type":4,"value":7}]
                        }]
                    }]
                },
                "token": "interaction-token",
                "version": 1
            }),
        })
    }

    fn spawned(outcome: DispatchOutcome) -> CommandTask {
        match outcome {
            DispatchOutcome::Spawned(task) => task,
            other => panic!("expected spawned command, got {other:?}"),
        }
    }
}
