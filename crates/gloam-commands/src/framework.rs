use std::sync::Arc;

use gloamwire::{
    RestClient,
    gateway::{GatewayEvent, GatewayIntents, ShardEvent, ShardId, ShardManager},
    model::{ApplicationCommandType, Interaction, InteractionType},
};
use tokio::{runtime::Handle, sync::Semaphore, task::JoinSet};

use crate::{
    CommandHandler, CommandRegistry, CommandTask, Context, DispatchOutcome, Error, Result, Runtime,
    SlashCommand,
};

/// Default upper bound for simultaneously executing command handlers.
pub const DEFAULT_MAX_CONCURRENT_COMMANDS: usize = 64;

/// Configured slash-command framework.
pub struct Framework<D> {
    data: Arc<D>,
    registry: CommandRegistry<D>,
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
}

impl<D> Framework<D>
where
    D: Send + Sync + 'static,
{
    /// Routes one Gloamwire Gateway event and spawns a matching slash command.
    ///
    /// Applications that own their Gateway loop can call this for each event.
    /// Events unrelated to chat-input application commands are returned as
    /// [`DispatchOutcome::Ignored`].
    pub fn dispatch(&self, rest: &RestClient, event: &GatewayEvent) -> Result<DispatchOutcome> {
        self.spawn_prepared(self.prepare_dispatch(rest, event, None)?)
    }

    /// Routes one sharded Gloamwire Gateway event while preserving shard identity.
    pub fn dispatch_shard(&self, rest: &RestClient, event: &ShardEvent) -> Result<DispatchOutcome> {
        self.spawn_prepared(self.prepare_dispatch(rest, &event.event, Some(event.shard_id))?)
    }

    /// Starts Gloamwire's recommended shard set and dispatches slash commands.
    ///
    /// The managed runtime requests no Gateway intents because Discord
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
                let handle = runtime.spawn(command.execute());
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

        let interaction: Interaction = serde_json::from_value(dispatch.data.clone())?;
        if interaction.kind != InteractionType::APPLICATION_COMMAND {
            return Ok(PreparedDispatch::Ignored);
        }

        let command_data = interaction
            .application_command_data()?
            .ok_or(Error::MissingApplicationCommandData)?;
        if command_data.kind != ApplicationCommandType::CHAT_INPUT {
            return Ok(PreparedDispatch::Ignored);
        }

        let Some(command) = self.registry.get(&command_data.name) else {
            return Ok(PreparedDispatch::Unregistered(command_data.name));
        };
        let descriptor = command.descriptor();
        let handler = command.handler();
        let runtime = Arc::new(self.runtime(rest.clone()));
        let context = Context::new(runtime, Arc::new(interaction), descriptor.name, shard_id);

        Ok(PreparedDispatch::Command(PreparedCommand {
            command_name: descriptor.name,
            handler,
            context,
            command_slots: Arc::clone(&self.command_slots),
        }))
    }

    async fn drive_shards(&self, rest: &RestClient, shards: &mut ShardManager) -> Result<()> {
        let mut commands = JoinSet::new();

        while let Some(event) = shards.next_event().await {
            reap_completed_commands(&mut commands)?;
            let event = event?;

            if let PreparedDispatch::Command(command) =
                self.prepare_dispatch(rest, &event.event, Some(event.shard_id))?
            {
                commands.spawn(command.execute());
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
    max_concurrent_commands: usize,
}

impl<D> FrameworkBuilder<D> {
    /// Creates a framework builder from application state.
    #[must_use]
    pub const fn new(data: D) -> Self {
        Self {
            data,
            commands: Vec::new(),
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
            max_concurrent_commands: self.max_concurrent_commands,
            command_slots: Arc::new(Semaphore::new(self.max_concurrent_commands)),
        })
    }
}

struct PreparedCommand<D> {
    command_name: &'static str,
    handler: CommandHandler<D>,
    context: Context<D>,
    command_slots: Arc<Semaphore>,
}

impl<D> PreparedCommand<D>
where
    D: Send + Sync + 'static,
{
    async fn execute(self) -> Result<()> {
        let _permit = self
            .command_slots
            .acquire_owned()
            .await
            .map_err(|_| Error::CommandSchedulerClosed)?;
        (self.handler)(self.context).await
    }
}

enum PreparedDispatch<D> {
    Ignored,
    Unregistered(String),
    Command(PreparedCommand<D>),
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
    use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

    use gloamwire::{
        RestClient,
        gateway::{DispatchEvent, GatewayEvent, ShardEvent, ShardId},
    };
    use tokio::sync::Semaphore;

    use crate::{
        CommandDescriptor, CommandTask, Context, DispatchOutcome, Error, Framework, Result,
        SlashCommand,
    };

    use super::DEFAULT_MAX_CONCURRENT_COMMANDS;

    static PING: CommandDescriptor = CommandDescriptor::new("ping", "Check bot responsiveness");
    static SLOW: CommandDescriptor = CommandDescriptor::new("slow", "Exercise concurrency limits");

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
        assert_eq!(
            framework.max_concurrent_commands(),
            DEFAULT_MAX_CONCURRENT_COMMANDS
        );
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
    async fn bounds_simultaneous_command_execution() -> Result<()> {
        let framework = Framework::builder(ConcurrencyState::new())
            .command(SlashCommand::new(&SLOW, slow_handler))
            .max_concurrent_commands(1)
            .build()?;
        let rest = RestClient::new("test-token")?;
        let first = spawned(framework.dispatch(&rest, &interaction_event("slow"))?);
        let second = spawned(framework.dispatch(&rest, &interaction_event("slow"))?);

        while framework.data().entered.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }

        assert_eq!(framework.data().max_active.load(Ordering::SeqCst), 1);
        framework.data().release.add_permits(2);
        first.join().await?;
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

    fn spawned(outcome: DispatchOutcome) -> CommandTask {
        match outcome {
            DispatchOutcome::Spawned(task) => task,
            other => panic!("expected spawned command, got {other:?}"),
        }
    }
}
