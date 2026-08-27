use std::sync::Arc;

use gloamwire::{RestClient, gateway::ShardId, model::Interaction};
use tokio::sync::Mutex;

use crate::{Runtime, response::ResponseState};

/// Per-execution context passed to slash-command handlers.
pub struct Context<D> {
    runtime: Arc<Runtime<D>>,
    interaction: Arc<Interaction>,
    command_name: &'static str,
    shard_id: Option<ShardId>,
    response_state: Arc<Mutex<ResponseState>>,
}

impl<D> Context<D> {
    pub(crate) fn new(
        runtime: Arc<Runtime<D>>,
        interaction: Arc<Interaction>,
        command_name: &'static str,
        shard_id: Option<ShardId>,
    ) -> Self {
        Self {
            runtime,
            interaction,
            command_name,
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

    /// Returns the registered slash-command name being executed.
    #[must_use]
    pub const fn command_name(&self) -> &'static str {
        self.command_name
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
            command_name: self.command_name,
            shard_id: self.shard_id,
            response_state: Arc::clone(&self.response_state),
        }
    }
}
