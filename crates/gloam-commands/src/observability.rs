/// Records that normal command execution entered the lifecycle pipeline.
pub(crate) fn command_started(path: &[&'static str]) {
    #[cfg(feature = "tracing")]
    tracing::debug!(command_path = ?path, "slash command execution started");

    #[cfg(not(feature = "tracing"))]
    let _ = path;
}

/// Records successful completion of the command lifecycle.
pub(crate) fn command_succeeded(path: &[&'static str]) {
    #[cfg(feature = "tracing")]
    tracing::debug!(command_path = ?path, "slash command execution completed");

    #[cfg(not(feature = "tracing"))]
    let _ = path;
}

/// Records a command execution failure without serializing the error or interaction payload.
pub(crate) fn command_failed(path: &[&'static str]) {
    #[cfg(feature = "tracing")]
    tracing::warn!(command_path = ?path, "slash command execution failed");

    #[cfg(not(feature = "tracing"))]
    let _ = path;
}

/// Records that the configured centralized error handler handled an execution error.
pub(crate) fn command_error_handled(path: &[&'static str]) {
    #[cfg(feature = "tracing")]
    tracing::debug!(command_path = ?path, "slash command error handled");

    #[cfg(not(feature = "tracing"))]
    let _ = path;
}

/// Records failure of the centralized error handler without serializing either error value.
pub(crate) fn command_error_handler_failed(path: &[&'static str]) {
    #[cfg(feature = "tracing")]
    tracing::warn!(command_path = ?path, "slash command error handler failed");

    #[cfg(not(feature = "tracing"))]
    let _ = path;
}
