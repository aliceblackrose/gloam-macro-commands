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
    runtime: Arc<Runtime<D>>,
    interaction: Arc<Interaction>,
    command_data: Arc<ApplicationCommandInteractionData>,
    command_path: Arc<[&'static str]>,
    command_options: Arc<[ApplicationCommandInteractionDataOption]>,
    focused_index: usize,
    shard_id: Option<ShardId>,
}

impl<D> AutocompleteContext<D> {
    pub(crate) fn new(
        runtime: Arc<Runtime<D>>,
        interaction: Arc<Interaction>,
        command_data: Arc<ApplicationCommandInteractionData>,
        command_path: Vec<&'static str>,
        command_options: Vec<ApplicationCommandInteractionDataOption>,
        focused_index: usize,
        shard_id: Option<ShardId>,
    ) -> Self {
        Self {
            runtime,
            interaction,
            command_data,
            command_path: command_path.into(),
            command_options: command_options.into(),
            focused_index,
            shard_id,
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

    /// Returns application-owned shared state.
    #[must_use]
    pub fn data(&self) -> &D {
        self.runtime.data()
    }

    /// Returns the Discord autocomplete interaction.
    #[must_use]
    pub fn interaction(&self) -> &Interaction {
        &self.interaction
    }

    /// Returns parsed top-level application-command data for this interaction.
    #[must_use]
    pub fn command_data(&self) -> &ApplicationCommandInteractionData {
        &self.command_data
    }

    /// Returns the resolved command path from top-level command to leaf.
    #[must_use]
    pub fn command_path(&self) -> &[&'static str] {
        &self.command_path
    }

    /// Returns typed extraction access to the resolved leaf option scope.
    ///
    /// Callers should inspect [`Self::focused_option`] directly for the focused
    /// partial value, because an incomplete value may not satisfy normal command
    /// extraction semantics yet.
    #[must_use]
    pub fn command_options(&self) -> CommandOptions<'_> {
        CommandOptions::from_slice(&self.command_options)
    }

    /// Returns the single option currently focused by Discord.
    #[must_use]
    pub fn focused_option(&self) -> &ApplicationCommandInteractionDataOption {
        &self.command_options[self.focused_index]
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
        self.command_path[0]
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
            runtime: Arc::clone(&self.runtime),
            interaction: Arc::clone(&self.interaction),
            command_data: Arc::clone(&self.command_data),
            command_path: Arc::clone(&self.command_path),
            command_options: Arc::clone(&self.command_options),
            focused_index: self.focused_index,
            shard_id: self.shard_id,
        }
    }
}
