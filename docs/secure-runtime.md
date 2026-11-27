# Secure runtime migration

## Security defaults

- Command execution defaults to a 120-second cooperative timeout, including validation, policy, handler and hooks. Autocomplete defaults to two seconds, including authorization and callback. Configure `FrameworkBuilder::command_timeout` and `autocomplete_timeout` as needed. Manual runtimes must enable Tokio's time driver. Timeouts cancel the in-flight future and return `Error::ExecutionTimeout`; they cannot preempt synchronous blocking code or tasks independently spawned by handlers. After hooks and error handlers are not guaranteed to run on cancellation.
- Autocomplete uses the normal command's context and member/application permission restrictions. Commands with custom checks require an explicit autocomplete-specific check; otherwise autocomplete is denied before invoking the handler. Use `#[command(..., autocomplete_check = can_complete)]` where `can_complete` is an async function taking `AutocompleteContext<D>` and returning `Result<bool>`. Manual policy builders accept a boxed-future function through `CommandPolicy::autocomplete_check`. Autocomplete does not reserve cooldowns.
- Standard reply, edit and followup helpers suppress parsed mentions. Call `Context::reply_with_mentions` with an explicit `AllowedMentions` value for intentional pings.
- Normal command submissions are validated against the static descriptor before executing custom checks, reserving cooldown or starting the handler. Unknown or duplicate options, invalid shapes, missing required values, type mismatches, bounds and fixed choices are rejected. Autocomplete partial values intentionally are not subjected to normal value constraints.

## Runtime internals

Contexts share one Arc-owned inner allocation; command paths are stored inline, and leaf options borrow the owned parsed command data. Dispatch deserializes from borrowed JSON, avoiding intermediate whole-value clones. Internal scheduler futures are no longer boxed. The managed runtime drains active tasks for up to five seconds on normal event-stream termination, then aborts unfinished tasks. Transport or registration failure still takes the error shutdown path.

The timeouts and safer mention defaults are deliberate behavioral changes. The framework does not automatically defer interactions; handlers must still acknowledge Discord within its initial interaction deadline. Timeout or permission errors do not automatically send an interaction response. For live deployments, use an appropriate error handler and validate real Discord transport behavior.

## Verification

Regression coverage includes integral JSON NUMBER decoding, option validation, timeout permit reclamation and fail-closed autocomplete authorization. The performance changes remove identifiable allocations and copies; throughput and latency require benchmarks.
