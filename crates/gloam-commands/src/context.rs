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
    inner: Arc<ContextInner<D>>,
    shard_id: Option<ShardId>,
}

struct ContextInner<D> {
    runtime: Runtime<D>,
    interaction: Interaction,
    command_data: ApplicationCommandInteractionData,
    command_path: [&'static str; 3],
    option_depth: usize,
    response_state: Mutex<ResponseState>,
}

impl<D> Context<D> {
    pub(crate) fn new(
        runtime: Runtime<D>,
        interaction: Interaction,
        command_data: ApplicationCommandInteractionData,
        command_path: Vec<&'static str>,
        option_depth: usize,
        shard_id: Option<ShardId>,
    ) -> Self {
        let mut path = [""; 3];
        path[..command_path.len()].copy_from_slice(&command_path);
        Self {
            inner: Arc::new(ContextInner {
                runtime,
                interaction,
                command_data,
                command_path: path,
                option_depth,
                response_state: Mutex::new(ResponseState::Pending),
            }),
            shard_id,
        }
    }

    /// Returns the shared command runtime.
    #[must_use]
    pub fn runtime(&self) -> &Runtime<D> {
        &self.inner.runtime
    }

    /// Returns the Gloamwire REST client.
    #[must_use]
    pub fn rest(&self) -> &RestClient {
        self.inner.runtime.rest()
    }

    /// Returns the application state.
    #[must_use]
    pub fn data(&self) -> &D {
        self.inner.runtime.data()
    }

    /// Returns the Discord interaction that invoked this command.
    #[must_use]
    pub fn interaction(&self) -> &Interaction {
        &self.inner.interaction
    }

    /// Returns parsed top-level application-command data for this invocation.
    #[must_use]
    pub fn command_data(&self) -> &ApplicationCommandInteractionData {
        &self.inner.command_data
    }

    /// Returns the resolved command path from top-level command to leaf handler.
    #[must_use]
    pub fn command_path(&self) -> &[&'static str] {
        &self.inner.command_path[..=self.inner.option_depth]
    }

    /// Returns typed extraction access to the resolved leaf option scope.
    #[must_use]
    pub fn command_options(&self) -> CommandOptions<'_> {
        CommandOptions::from_slice(self.leaf_options())
    }

    fn leaf_options(&self) -> &[ApplicationCommandInteractionDataOption] {
        crate::options::leaf_options(&self.inner.command_data, self.inner.option_depth)
    }

    /// Returns the registered top-level slash-command name being executed.
    #[must_use]
    pub fn command_name(&self) -> &'static str {
        self.inner.command_path[0]
    }

    /// Returns the Gateway shard that received the interaction when known.
    #[must_use]
    pub const fn shard_id(&self) -> Option<ShardId> {
        self.shard_id
    }

    pub(crate) fn response_state(&self) -> &Mutex<ResponseState> {
        &self.inner.response_state
    }
}

impl<D> Clone for Context<D> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            shard_id: self.shard_id,
        }
    }
}
