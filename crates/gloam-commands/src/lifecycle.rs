use crate::{CommandFuture, Context, Error};

/// Framework-level hook invoked around normal slash-command handlers.
///
/// Hooks receive a cloned [`Context`], so response acknowledgement state and
/// application state remain shared with the command handler.
pub type CommandHook<D> = fn(Context<D>) -> CommandFuture;

/// Framework-level handler for command execution errors.
///
/// The handler receives ownership of the execution error. Returning `Ok(())`
/// marks the error as handled for the spawned command task. Returning an error
/// propagates that error from [`crate::CommandTask::join`].
pub type CommandErrorHandler<D> = fn(Context<D>, Error) -> CommandFuture;
