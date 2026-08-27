use std::{future::Future, pin::Pin};

use gloamwire::model::{ApplicationCommandNumericValue, ApplicationCommandOptionType};

use crate::{Context, Result};

/// Boxed future returned by generated slash-command adapters.
pub type CommandFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

/// Erased handler function stored in the command registry.
pub type CommandHandler<D> = fn(Context<D>) -> CommandFuture;

/// Static metadata describing one Discord chat-input command option.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandOptionDescriptor {
    /// Option name registered with Discord.
    pub name: &'static str,
    /// Human-readable option description registered with Discord.
    pub description: &'static str,
    /// Discord option kind generated from the Rust parameter type.
    pub kind: ApplicationCommandOptionType,
    /// Whether Discord requires this option in an invocation.
    pub required: bool,
    /// Optional minimum numeric value.
    pub min_value: Option<ApplicationCommandNumericValue>,
    /// Optional maximum numeric value.
    pub max_value: Option<ApplicationCommandNumericValue>,
    /// Optional minimum string length.
    pub min_length: Option<u32>,
    /// Optional maximum string length.
    pub max_length: Option<u32>,
}

impl CommandOptionDescriptor {
    /// Creates an unconstrained command-option descriptor.
    #[must_use]
    pub const fn new(
        name: &'static str,
        description: &'static str,
        kind: ApplicationCommandOptionType,
        required: bool,
    ) -> Self {
        Self {
            name,
            description,
            kind,
            required,
            min_value: None,
            max_value: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Sets the minimum numeric value.
    #[must_use]
    pub const fn min_value(mut self, value: ApplicationCommandNumericValue) -> Self {
        self.min_value = Some(value);
        self
    }

    /// Sets an integer minimum without exposing Gloamwire in generated code.
    #[must_use]
    pub const fn min_integer(self, value: i64) -> Self {
        self.min_value(ApplicationCommandNumericValue::Integer(value))
    }

    /// Sets a number minimum without exposing Gloamwire in generated code.
    #[must_use]
    pub const fn min_number(self, value: f64) -> Self {
        self.min_value(ApplicationCommandNumericValue::Number(value))
    }

    /// Sets the maximum numeric value.
    #[must_use]
    pub const fn max_value(mut self, value: ApplicationCommandNumericValue) -> Self {
        self.max_value = Some(value);
        self
    }

    /// Sets an integer maximum without exposing Gloamwire in generated code.
    #[must_use]
    pub const fn max_integer(self, value: i64) -> Self {
        self.max_value(ApplicationCommandNumericValue::Integer(value))
    }

    /// Sets a number maximum without exposing Gloamwire in generated code.
    #[must_use]
    pub const fn max_number(self, value: f64) -> Self {
        self.max_value(ApplicationCommandNumericValue::Number(value))
    }

    /// Sets the minimum string length.
    #[must_use]
    pub const fn min_length(mut self, value: u32) -> Self {
        self.min_length = Some(value);
        self
    }

    /// Sets the maximum string length.
    #[must_use]
    pub const fn max_length(mut self, value: u32) -> Self {
        self.max_length = Some(value);
        self
    }
}

/// Static metadata describing a Discord chat-input command or group node.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CommandDescriptor {
    /// Command or group name registered with Discord.
    pub name: &'static str,
    /// Human-readable description registered with Discord.
    pub description: &'static str,
    /// Scalar options generated from a leaf handler signature.
    pub options: &'static [CommandOptionDescriptor],
}

impl CommandDescriptor {
    /// Creates a command descriptor without scalar options.
    #[must_use]
    pub const fn new(name: &'static str, description: &'static str) -> Self {
        Self {
            name,
            description,
            options: &[],
        }
    }

    /// Attaches generated scalar command-option descriptors.
    #[must_use]
    pub const fn with_options(mut self, options: &'static [CommandOptionDescriptor]) -> Self {
        self.options = options;
        self
    }
}

enum SlashCommandKind<D> {
    Leaf(CommandHandler<D>),
    Group(Vec<SlashCommand<D>>),
}

/// One node in the registered Discord chat-input command tree.
///
/// A top-level node can be a leaf command or a group. A group may contain leaf
/// subcommands and one additional level of subcommand groups, matching Discord's
/// native hierarchy.
pub struct SlashCommand<D> {
    descriptor: &'static CommandDescriptor,
    kind: SlashCommandKind<D>,
}

impl<D> SlashCommand<D> {
    /// Creates a leaf slash command from static metadata and an erased handler.
    #[must_use]
    pub const fn new(descriptor: &'static CommandDescriptor, handler: CommandHandler<D>) -> Self {
        Self {
            descriptor,
            kind: SlashCommandKind::Leaf(handler),
        }
    }

    /// Creates a group node containing child commands.
    #[must_use]
    pub fn group(descriptor: &'static CommandDescriptor, children: Vec<Self>) -> Self {
        Self {
            descriptor,
            kind: SlashCommandKind::Group(children),
        }
    }

    /// Returns the static node descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &'static CommandDescriptor {
        self.descriptor
    }

    /// Returns the erased handler when this node is a leaf command.
    #[must_use]
    pub const fn handler(&self) -> Option<CommandHandler<D>> {
        match &self.kind {
            SlashCommandKind::Leaf(handler) => Some(*handler),
            SlashCommandKind::Group(_) => None,
        }
    }

    /// Returns child nodes when this node is a group.
    #[must_use]
    pub fn children(&self) -> &[Self] {
        match &self.kind {
            SlashCommandKind::Leaf(_) => &[],
            SlashCommandKind::Group(children) => children,
        }
    }

    /// Returns whether this node is a leaf command.
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        matches!(self.kind, SlashCommandKind::Leaf(_))
    }

    /// Returns whether this node is a group.
    #[must_use]
    pub const fn is_group(&self) -> bool {
        matches!(self.kind, SlashCommandKind::Group(_))
    }
}
