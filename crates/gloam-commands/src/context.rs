use std::sync::Arc;

use gloamwire::{
    RestClient,
    gateway::ShardId,
    model::{
        ApplicationCommandInteractionData, ApplicationCommandInteractionDataOption, Interaction,
    },
};
use tokio::sync::Mutex;

use crate::{CommandOptions, Runtime, response::ResponseState};

/// Per-execution context passed to slash-command handlers.
pub struct Context<D> {
    runtime: Arc<Runtime<D>>,
    interaction: Arc<Interaction>,
    command_data: Arc<ApplicationCommandInteractionData>,
    command_path: Arc<[&'static str]>,
    command_options: Arc<[ApplicationCommandInteractionDataOption]>,
    shard_id: Option<ShardId>,
    response_state: Arc<Mutex<ResponseState>>,
}

impl<D> Context<D> {
    pub(crate) fn new(
        runtime: Arc<Runtime<D>>,
        interaction: Arc<Interaction>,
        command_data: Arc<ApplicationCommandInteractionData>,
        command_path: Vec<&'static str>,
        command_options: Vec<ApplicationCommandInteractionDataOption>,
        shard_id: Option<ShardId>,
    ) -> Self {
        Self {
            runtime,
            interaction,
            command_data,
            command_path: command_path.into(),
            command_options: command_options.into(),
            shard_id,
            response_state: Arc::new(Mutex::new(ResponseState::Pending)),
        }
    }

    /// Returns the shared command runtime.
    #[must_use]
    pub fn runtime(&self) -> &Runtime<D> {
        &self.runtime
    }

    /// Returns the Gloamwire REST client.
    #[must_use]
    pub fn rest(&self) -> &RestClient {
        self.runtime.rest()
    }

    /// Returns the application state.
    #[must_use]
    pub fn data(&self) -> &D {
        self.runtime.data()
    }

    /// Returns the Discord interaction that invoked this command.
    #[must_use]
    pub fn interaction(&self) -> &Interaction {
        &self.interaction
    }

    /// Returns parsed top-level application-command data for this invocation.
    #[must_use]
    pub fn command_data(&self) -> &ApplicationCommandInteractionData {
        &self.command_data
    }

    /// Returns the resolved command path from top-level command to leaf handler.
    #[must_use]
    pub fn command_path(&self) -> &[&'static str] {
        &self.command_path
    }

    /// Returns typed extraction access to the resolved leaf option scope.
    #[must_use]
    pub fn command_options(&self) -> CommandOptions<'_> {
        CommandOptions::from_slice(&self.command_options)
    }

    /// Returns the registered top-level slash-command name being executed.
    #[must_use]
    pub fn command_name(&self) -> &'static str {
        self.command_path[0]
    }

    /// Returns the Gateway shard that received the interaction when known.
    #[must_use]
    pub const fn shard_id(&self) -> Option<ShardId> {
        self.shard_id
    }

    pub(crate) fn response_state(&self) -> &Mutex<ResponseState> {
        &self.response_state
    }
}

impl<D> Clone for Context<D> {
    fn clone(&self) -> Self {
        Self {
            runtime: Arc::clone(&self.runtime),
            interaction: Arc::clone(&self.interaction),
            command_data: Arc::clone(&self.command_data),
            command_path: Arc::clone(&self.command_path),
            command_options: Arc::clone(&self.command_options),
            shard_id: self.shard_id,
            response_state: Arc::clone(&self.response_state),
        }
    }
}
