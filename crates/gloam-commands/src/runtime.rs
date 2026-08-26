use std::sync::Arc;

use gloamwire::RestClient;

/// Shared resources available to every command context.
pub struct Runtime<D> {
    rest: Arc<RestClient>,
    data: Arc<D>,
}

impl<D> Runtime<D> {
    /// Creates a runtime from owned REST and application state values.
    #[must_use]
    pub fn new(rest: RestClient, data: D) -> Self {
        Self {
            rest: Arc::new(rest),
            data: Arc::new(data),
        }
    }

    /// Creates a runtime from already shared REST and application state values.
    #[must_use]
    pub fn from_shared(rest: Arc<RestClient>, data: Arc<D>) -> Self {
        Self { rest, data }
    }

    /// Returns the Gloamwire REST client.
    #[must_use]
    pub fn rest(&self) -> &RestClient {
        &self.rest
    }

    /// Returns the application state.
    #[must_use]
    pub fn data(&self) -> &D {
        &self.data
    }

    /// Returns a shared handle to the REST client.
    #[must_use]
    pub fn rest_arc(&self) -> Arc<RestClient> {
        Arc::clone(&self.rest)
    }

    /// Returns a shared handle to the application state.
    #[must_use]
    pub fn data_arc(&self) -> Arc<D> {
        Arc::clone(&self.data)
    }
}
