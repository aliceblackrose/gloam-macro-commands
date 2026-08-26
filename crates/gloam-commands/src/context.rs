use std::sync::Arc;

use gloamwire::RestClient;

use crate::Runtime;

/// Per-execution context passed to slash-command handlers.
///
/// Interaction-specific accessors and response helpers are added in later
/// phases once command dispatch is connected to Gloamwire interactions.
pub struct Context<D> {
    runtime: Arc<Runtime<D>>,
    command_name: &'static str,
}

impl<D> Context<D> {
    #[allow(dead_code)]
    pub(crate) fn new(runtime: Arc<Runtime<D>>, command_name: &'static str) -> Self {
        Self {
            runtime,
            command_name,
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

    /// Returns the registered slash-command name being executed.
    #[must_use]
    pub const fn command_name(&self) -> &'static str {
        self.command_name
    }
}

impl<D> Clone for Context<D> {
    fn clone(&self) -> Self {
        Self {
            runtime: Arc::clone(&self.runtime),
            command_name: self.command_name,
        }
    }
}
