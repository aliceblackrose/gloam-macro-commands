use std::fmt;

use tokio::task::JoinHandle;

use crate::{Error, Result};

/// Result of routing one Gateway event through the slash-command framework.
#[must_use]
pub enum DispatchOutcome {
    /// The Gateway event was unrelated to a registered chat-input command.
    Ignored,
    /// Discord invoked a chat-input command that is not registered locally.
    Unregistered {
        /// Top-level Discord command name that was not found.
        name: String,
    },
    /// A registered command was spawned for asynchronous execution.
    Spawned(CommandTask),
}

impl fmt::Debug for DispatchOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ignored => formatter.write_str("Ignored"),
            Self::Unregistered { name } => formatter
                .debug_struct("Unregistered")
                .field("name", name)
                .finish(),
            Self::Spawned(task) => formatter.debug_tuple("Spawned").field(task).finish(),
        }
    }
}

/// Handle for one asynchronously executing slash command.
#[must_use]
pub struct CommandTask {
    command_name: &'static str,
    handle: JoinHandle<Result<()>>,
}

impl CommandTask {
    pub(crate) const fn new(command_name: &'static str, handle: JoinHandle<Result<()>>) -> Self {
        Self {
            command_name,
            handle,
        }
    }

    /// Returns the registered command name associated with this task.
    #[must_use]
    pub const fn command_name(&self) -> &'static str {
        self.command_name
    }

    /// Returns whether the spawned command task has finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.is_finished()
    }

    /// Requests cancellation of the spawned command task.
    pub fn abort(&self) {
        self.handle.abort();
    }

    /// Waits for the command task and returns its handler result.
    pub async fn join(self) -> Result<()> {
        self.handle.await.map_err(Error::CommandTask)?
    }
}

impl fmt::Debug for CommandTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommandTask")
            .field("command_name", &self.command_name)
            .field("is_finished", &self.is_finished())
            .finish()
    }
}
