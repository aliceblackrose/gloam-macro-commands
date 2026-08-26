# Gloam Macro Commands

A focused Discord **slash-command framework** for [Gloamwire](https://github.com/cybellereaper/gloamwire), written for Rust 1.98 and Edition 2024.

> **Current status:** Phase 2 — zero-option `#[command]` macro.

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

## Workspace

```text
crates/
├── gloam-commands/         # runtime, context, registry, dispatch, responses
└── gloam-commands-macros/  # procedural macro generation only
```

The proc-macro crate contains no Discord runtime behavior. Generated code targets public runtime abstractions from `gloam-commands`, which in turn uses Gloamwire's Gateway, REST, and model APIs.

## Current API

Phase 2 removes the need to hand-write command descriptors and erased handler adapters for zero-option slash commands:

```rust,ignore
use gloam_commands::prelude::*;

struct State;

#[command(description = "Check bot responsiveness")]
async fn ping(_ctx: Context<State>) -> Result<()> {
    Ok(())
}

let framework = Framework::builder(State)
    .commands(commands![ping])
    .build()?;
```

`#[command]` currently:

- requires an `async fn`;
- requires exactly one `Context<D>` parameter;
- requires a slash-command description;
- supports an optional explicit `name = "..."`;
- validates Discord chat-input command naming and description length rules;
- preserves the original Rust function;
- generates the static descriptor and erased adapter used by `commands![...]`.

Typed slash-command parameters are intentionally not part of Phase 2. They are planned for Phase 5 after interaction dispatch and response handling establish the runtime semantics they depend on.

## Planned application API

The eventual managed runtime is intended to build on the same command declaration without changing handler metadata:

```rust,ignore
#[command(description = "Say hello")]
async fn hello(
    ctx: Context<State>,
    #[description = "Person to greet"] name: String,
) -> Result<()> {
    ctx.reply(format!("Hello, {name}!" )).await?;
    Ok(())
}

Framework::builder(State)
    .commands(commands![hello])
    .registration(Registration::Global)
    .run(std::env::var("DISCORD_TOKEN")?)
    .await?;
```

That example describes later roadmap phases; typed options, response helpers, command synchronization, and the managed Gateway runtime are not Phase 2 features.

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
