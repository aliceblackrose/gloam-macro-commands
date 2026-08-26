# Gloam Macro Commands

A focused Discord **slash-command framework** for [Gloamwire](https://github.com/cybellereaper/gloamwire), written for Rust 1.98 and Edition 2024.

> **Current status:** Phase 1 — runtime foundation.

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

## Target API

The macro surface begins in Phase 2. The intended application API is:

```rust,ignore
use gloam_commands::prelude::*;

struct State;

#[command(description = "Say hello")]
async fn hello(
    ctx: Context<State>,
    #[description = "Person to greet"] name: String,
) -> Result<()> {
    ctx.reply(format!("Hello, {name}!" )).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    Framework::builder(State)
        .commands(commands![hello])
        .registration(Registration::Global)
        .run(std::env::var("DISCORD_TOKEN")?)
        .await
}
```

That API is a roadmap target, not yet the Phase 1 feature set.

## Current Phase 1 API

Phase 1 establishes the ownership and registration model used by later generated code:

```rust,ignore
use gloam_commands::{CommandDescriptor, Context, Framework, SlashCommand};

static PING: CommandDescriptor =
    CommandDescriptor::new("ping", "Check bot responsiveness");

fn ping_adapter(_ctx: Context<State>) -> gloam_commands::CommandFuture {
    Box::pin(async { Ok(()) })
}

let framework = Framework::builder(State)
    .command(SlashCommand::new(&PING, ping_adapter))
    .build()?;
```

Application authors will not need to write descriptors or adapters manually once Phase 2 lands.

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
