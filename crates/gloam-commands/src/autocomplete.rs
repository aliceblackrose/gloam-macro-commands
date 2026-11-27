use std::{future::Future, pin::Pin, sync::Arc};

use gloamwire::{
    RestClient,
    gateway::ShardId,
    model::{
        ApplicationCommandInteractionData, ApplicationCommandInteractionDataOption,
        ApplicationCommandInteractionValue, Interaction,
    },
};

use crate::{CommandOptions, Result, Runtime};

/// Boxed future returned by generated autocomplete-handler adapters.
pub type AutocompleteFuture =
    Pin<Box<dyn Future<Output = Result<Vec<AutocompleteChoice>>> + Send + 'static>>;

/// Erased autocomplete handler stored on one leaf command option.
pub type AutocompleteHandler<D> = fn(AutocompleteContext<D>) -> AutocompleteFuture;

/// Owned scalar value returned by an autocomplete handler.
#[derive(Debug, Clone, PartialEq)]
pub enum AutocompleteChoiceValue {
    /// String autocomplete value.
    String(String),
    /// Integer autocomplete value.
    Integer(i64),
    /// Number autocomplete value.
    Number(f64),
}

/// One dynamic Discord autocomplete result.
#[derive(Debug, Clone, PartialEq)]
pub struct AutocompleteChoice {
    /// Human-readable choice name shown by Discord.
    pub name: String,
    /// Scalar value submitted if the user selects this result.
    pub value: AutocompleteChoiceValue,
}

impl AutocompleteChoice {
    /// Creates a string autocomplete choice.
    #[must_use]
    pub fn string(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: AutocompleteChoiceValue::String(value.into()),
        }
    }

    /// Creates an integer autocomplete choice.
    #[must_use]
    pub fn integer(name: impl Into<String>, value: i64) -> Self {
        Self {
            name: name.into(),
            value: AutocompleteChoiceValue::Integer(value),
        }
    }

    /// Creates a number autocomplete choice.
    #[must_use]
    pub fn number(name: impl Into<String>, value: f64) -> Self {
        Self {
            name: name.into(),
            value: AutocompleteChoiceValue::Number(value),
        }
    }
}

/// Generated association between one command option and its autocomplete handler.
#[derive(Clone, Copy)]
pub struct AutocompleteHandlerDescriptor<D> {
    option_name: &'static str,
    handler: AutocompleteHandler<D>,
}

impl<D> AutocompleteHandlerDescriptor<D> {
    /// Creates an autocomplete-handler association for one registered option name.
    #[must_use]
    pub const fn new(option_name: &'static str, handler: AutocompleteHandler<D>) -> Self {
        Self {
            option_name,
            handler,
        }
    }

    pub(crate) const fn option_name(&self) -> &'static str {
        self.option_name
    }

    pub(crate) const fn handler(&self) -> AutocompleteHandler<D> {
        self.handler
    }
}

/// Per-interaction context passed to autocomplete handlers.
///
/// Autocomplete has different acknowledgement semantics from command execution,
/// so this type deliberately does not expose normal interaction response helpers.
pub struct AutocompleteContext<D> {
    inner: Arc<AutocompleteContextInner<D>>,
    shard_id: Option<ShardId>,
}

struct AutocompleteContextInner<D> {
    runtime: Runtime<D>,
    interaction: Interaction,
    command_data: ApplicationCommandInteractionData,
    command_path: [&'static str; 3],
    option_depth: usize,
    focused_index: usize,
}

impl<D> AutocompleteContext<D> {
    pub(crate) fn new(
        runtime: Runtime<D>,
        interaction: Interaction,
        command_data: ApplicationCommandInteractionData,
        command_path: Vec<&'static str>,
        option_depth: usize,
        focused_index: usize,
        shard_id: Option<ShardId>,
    ) -> Self {
        let mut path = [""; 3];
        path[..command_path.len()].copy_from_slice(&command_path);
        Self {
            inner: Arc::new(AutocompleteContextInner {
                runtime,
                interaction,
                command_data,
                command_path: path,
                option_depth,
                focused_index,
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

    /// Returns application-owned shared state.
    #[must_use]
    pub fn data(&self) -> &D {
        self.inner.runtime.data()
    }

    /// Returns the Discord autocomplete interaction.
    #[must_use]
    pub fn interaction(&self) -> &Interaction {
        &self.inner.interaction
    }

    /// Returns parsed top-level application-command data for this interaction.
    #[must_use]
    pub fn command_data(&self) -> &ApplicationCommandInteractionData {
        &self.inner.command_data
    }

    /// Returns the resolved command path from top-level command to leaf.
    #[must_use]
    pub fn command_path(&self) -> &[&'static str] {
        &self.inner.command_path[..=self.inner.option_depth]
    }

    /// Returns typed extraction access to the resolved leaf option scope.
    ///
    /// Callers should inspect [`Self::focused_option`] directly for the focused
    /// partial value, because an incomplete value may not satisfy normal command
    /// extraction semantics yet.
    #[must_use]
    pub fn command_options(&self) -> CommandOptions<'_> {
        CommandOptions::from_slice(self.leaf_options())
    }

    /// Returns the single option currently focused by Discord.
    #[must_use]
    pub fn focused_option(&self) -> &ApplicationCommandInteractionDataOption {
        &self.leaf_options()[self.inner.focused_index]
    }

    fn leaf_options(&self) -> &[ApplicationCommandInteractionDataOption] {
        crate::options::leaf_options(&self.inner.command_data, self.inner.option_depth)
    }

    /// Returns the registered name of the focused option.
    #[must_use]
    pub fn focused_name(&self) -> &str {
        &self.focused_option().name
    }

    /// Returns the current partial value for the focused option.
    #[must_use]
    pub fn focused_value(&self) -> Option<&ApplicationCommandInteractionValue> {
        self.focused_option().value.as_ref()
    }

    /// Returns the registered top-level command name.
    #[must_use]
    pub fn command_name(&self) -> &'static str {
        self.inner.command_path[0]
    }

    /// Returns the Gateway shard that received the interaction when known.
    #[must_use]
    pub const fn shard_id(&self) -> Option<ShardId> {
        self.shard_id
    }
}

impl<D> Clone for AutocompleteContext<D> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            shard_id: self.shard_id,
        }
    }
}
