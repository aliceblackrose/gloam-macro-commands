# Hooks and observability

Phase 11 adds framework-level lifecycle hooks, centralized command-error handling, and optional `tracing` instrumentation for normal slash-command execution.

## Lifecycle order

Normal command execution uses one deterministic pipeline:

1. Command execution policy is evaluated.
2. `before_command` hooks run in registration order.
3. The generated slash-command handler runs.
4. `after_command` hooks run in registration order after a started handler.
5. The first execution error, if any, is routed to the configured `command_error_handler`.

A failing policy or before hook prevents the user handler from starting, so after hooks do not run in that case. Once the user handler starts, every registered after hook is given an opportunity to run, including when the handler returns an error. If multiple execution steps fail, the first error is preserved.

Autocomplete interactions intentionally use their existing independent execution and response path; normal command hooks and policy do not run for autocomplete.

## Hook signatures

Hooks use the same erased future ABI as generated command handlers:

```rust,ignore
use gloam_commands::{CommandFuture, Context};

fn before<D>(ctx: Context<D>) -> CommandFuture
where
    D: Send + Sync + 'static,
{
    Box::pin(async move {
        let _path = ctx.command_path();
        Ok(())
    })
}
```

`Context` is cloneable and all clones share the same runtime and acknowledgement state. A hook can therefore use the normal response helpers without creating a second interaction state machine.

## Centralized command errors

A command-error handler receives the command context plus ownership of the first execution error:

```rust,ignore
use gloam_commands::{CommandFuture, Context, Error};

fn on_error<D>(ctx: Context<D>, error: Error) -> CommandFuture
where
    D: Send + Sync + 'static,
{
    Box::pin(async move {
        eprintln!("command {} failed: {error}", ctx.command_path().join(" "));
        Ok(())
    })
}
```

Returning `Ok(())` marks the execution error as handled for the spawned command task. Returning an error propagates that error from `CommandTask::join()`.

The framework does not install a default user-facing error response. Applications remain responsible for deciding whether failures should become an ephemeral reply, structured application telemetry, or another response policy.

## Optional tracing

Enable the `tracing` feature when the application already uses the `tracing` ecosystem:

```toml
[dependencies]
gloam-commands = { version = "0.1", features = ["tracing"] }
```

The feature enables Gloamwire's matching tracing integration and emits framework lifecycle events. The framework never installs or configures a subscriber; the application keeps ownership of subscriber selection, filters, formatting, and output destinations.

Framework lifecycle events currently expose only the resolved static command path. They deliberately do **not** accept or record:

- interaction tokens;
- raw Gateway payloads;
- submitted command option values;
- autocomplete partial values;
- response bodies;
- application state.

This restriction is enforced structurally by the internal observability API: lifecycle logging helpers receive only `&[&'static str]`. The framework also avoids formatting execution errors into its built-in tracing events because downstream error variants can contain user-controlled option names or transport details. Applications that log errors in a custom `command_error_handler` should apply their own data-classification and redaction policy.
