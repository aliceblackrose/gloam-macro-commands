# Manual dispatch integration

`Framework::run(...)` is the managed path: it creates the Gloamwire REST client, starts the recommended shard set, optionally synchronizes commands after the first `READY`, and dispatches interactions.

Applications that already own their Gloamwire Gateway loop should keep that loop and call `Framework::dispatch(...)` or `Framework::dispatch_shard(...)` for each event instead of starting a second Gateway connection.

## Existing shard loop

```rust,ignore
use gloam_commands::prelude::*;
use gloamwire::{
    RestClient,
    gateway::{GatewayIntents, ShardManager},
};

# struct State;
# fn build_framework() -> Result<Framework<State>> {
#     Framework::builder(State).build()
# }
# async fn run() -> Result<()> {
let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN must be set");
let rest = RestClient::new(&token)?;
let framework = build_framework()?;
let mut shards = ShardManager::start(token, GatewayIntents::empty(), &rest).await?;

while let Some(event) = shards.next_event().await {
    let event = event?;

    match framework.dispatch_shard(&rest, &event)? {
        DispatchOutcome::Ignored | DispatchOutcome::Unregistered { .. } => {}
        DispatchOutcome::AtCapacity { name } => {
            eprintln!("command `{name}` was rejected at capacity");
        }
        DispatchOutcome::Spawned(task) => {
            // `dispatch_shard` has already spawned the command task. Dropping the
            // handle detaches it, so do not await it inline if the Gateway loop
            // must continue polling without waiting for command completion.
            tokio::spawn(async move {
                if let Err(error) = task.join().await {
                    eprintln!("command task failed: {error}");
                }
            });
        }
    }
}
# Ok(())
# }
```

Use `dispatch(...)` instead when the application works with unsharded `GatewayEvent` values. Use `dispatch_shard(...)` when a `ShardEvent` is available so `Context::shard_id()` and `AutocompleteContext::shard_id()` retain the receiving shard.

## Registration ownership

Manual dispatch does not implicitly synchronize commands. This keeps registration independent from applications that already manage their own startup lifecycle.

If the framework should own command registration, configure a `Registration` target and call `Framework::synchronize_commands(...)` once after the application ID is known:

```rust,ignore
framework
    .synchronize_commands(&rest, application_id)
    .await?;
```

`Registration::None` remains the default and performs no Discord registration requests.

## Concurrency behavior

`dispatch(...)` and `dispatch_shard(...)` reserve execution capacity before spawning a command or autocomplete task. When the framework-wide limit, or a leaf command's `max_concurrency` limit, has no immediately available permit, dispatch returns `DispatchOutcome::AtCapacity` rather than creating a queued waiter task.

Normal command execution policy is evaluated inside the spawned task before the user handler. Autocomplete follows its own response path and does not run normal command execution policy.

## Error ownership

Errors that occur while parsing or preparing an interaction are returned directly by `dispatch(...)` / `dispatch_shard(...)`. Errors produced after a handler is spawned are returned by `CommandTask::join()`.

Until a centralized command-error handler is configured in Phase 11, applications that need visibility into handler failures should retain or join returned `CommandTask` handles asynchronously rather than dropping every handle.
