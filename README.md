# Gloam Macro Commands

A focused Discord **slash-command framework** for [Gloamwire](https://github.com/cybellereaper/gloamwire), written for Rust 1.98 and Edition 2024.

> **Current status:** Phase 3 — interaction dispatch and managed runtime.

The framework is intentionally limited to Discord chat-input application commands. It does **not** implement prefix commands, message-content parsing, or a hybrid prefix/slash command abstraction.

## Design goals

- Generate Discord command metadata from typed Rust handlers.
- Generate runtime option extraction from the same handler signature.
- Provide an owned `Context<D>` with shared application state and Gloamwire access.
- Keep Gateway and REST protocol behavior in Gloamwire instead of wrapping it with parallel implementations.
- Support both a managed runtime and manual dispatch for applications that own their Gateway loop.
- Keep registration explicit and deterministic.
- Produce useful compile-time diagnostics for invalid command signatures.
- Keep command execution from blocking Gateway polling.
- Bound framework-owned command tasks instead of accumulating unbounded scheduler waiters.

## Workspace

```text
crates/
├── gloam-commands/         # runtime, context, registry, dispatch, responses
└── gloam-commands-macros/  # procedural macro generation only
```

The proc-macro crate contains no Discord runtime behavior. Generated code targets public runtime abstractions from `gloam-commands`, which in turn uses Gloamwire's Gateway, REST, sharding, and model APIs.

## Current API

Zero-option slash commands can be declared with `#[command]` and executed through the managed Gloamwire shard runtime:

```rust,ignore
use gloam_commands::prelude::*;

struct State;

#[command(description = "Check bot responsiveness")]
async fn ping(ctx: Context<State>) -> Result<()> {
    println!("interaction {}", ctx.interaction().id.get());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let framework = Framework::builder(State)
        .commands(commands![ping])
        .max_concurrent_commands(64)
        .build()?;

    framework.run(std::env::var("DISCORD_TOKEN").unwrap()).await
}
```

The managed runtime:

- uses Gloamwire's recommended `ShardManager` configuration;
- requests no Gateway intents because application-command interactions do not require one;
- parses only `INTERACTION_CREATE` application-command payloads;
- routes only chat-input commands registered in the local command registry;
- preserves the receiving `ShardId` in `Context<D>`;
- reserves an execution slot before spawning a handler, so command tasks remain bounded;
- continues polling the Gateway while command handlers execute.

Applications that already own a Gloamwire Gateway loop can call `Framework::dispatch(...)` or `Framework::dispatch_shard(...)` instead. Manual dispatch returns `DispatchOutcome`, including `Ignored`, `Unregistered`, `AtCapacity`, and a `Spawned(CommandTask)` handle.

`#[command]` currently:

- requires an `async fn`;
- requires exactly one `Context<D>` parameter;
- requires a `Result<()>` return type;
- requires a slash-command description;
- supports an optional explicit `name = "..."`;
- validates Discord chat-input command naming and description length rules;
- preserves the original Rust function;
- generates the static descriptor and erased adapter used by `commands![...]`.

Command synchronization is not implemented yet. Until Phase 6, Discord application commands must already be registered externally for managed Gateway dispatch to receive invocations.

Typed slash-command parameters are also not implemented yet. They are planned for Phase 5 after Phase 4 establishes interaction response semantics.

## Planned application API

Later phases will extend the same command declaration with typed options, responses, and generated command synchronization:

```rust,ignore
#[command(description = "Say hello")]
async fn hello(
    ctx: Context<State>,
    #[description = "Person to greet"] name: String,
) -> Result<()> {
    ctx.reply(format!("Hello, {name}!" )).await?;
    Ok(())
}

let framework = Framework::builder(State)
    .commands(commands![hello])
    .registration(Registration::Global)
    .build()?;

framework.run(std::env::var("DISCORD_TOKEN")?).await?;
```

That example describes later roadmap phases; typed options, response helpers, and command synchronization are not Phase 3 features.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the phase-by-phase implementation plan and exit criteria.

The major phases are:

1. runtime foundation;
2. `#[command]` macro;
3. interaction dispatch and managed runtime;
4. interaction responses;
5. typed slash-command options;
6. command synchronization;
7. subcommands and groups;
8. choices and typed choice enums;
9. autocomplete;
10. checks and execution policy;
11. hooks, observability, and 0.1 hardening.

See [ARCHITECTURE.md](ARCHITECTURE.md) for crate boundaries, ownership rules, concurrency design, and explicit non-goals.

## Non-goals

Gloam Macro Commands will not provide:

- `!ping`-style prefix commands;
- message-content argument parsing;
- prefix aliases or prefix configuration;
- a message-command tokenizer/lexer;
- a compatibility abstraction that treats prefix and slash commands as the same invocation type;
- a replacement for Gloamwire's Gateway, REST, rate-limit, sharding, or Discord model implementations.

## License

MIT
