# Gloam Macro Commands

A focused Discord **slash-command framework** for [Gloamwire](https://github.com/cybellereaper/gloamwire), written for Rust 1.98 and Edition 2024.

> **Current status:** Phase 5 — typed slash-command options.

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
- Preserve Discord interaction acknowledgement rules across cloned command contexts.

## Workspace

```text
crates/
├── gloam-commands/         # runtime, context, registry, dispatch, responses, options
└── gloam-commands-macros/  # procedural macro generation only
```

The proc-macro crate contains no Discord runtime behavior. Generated code targets public runtime abstractions from `gloam-commands`, which in turn uses Gloamwire's Gateway, REST, sharding, interaction, and model APIs.

## Current API

Typed slash commands can be declared with `#[command]`, respond through `Context<D>`, and execute through the managed Gloamwire shard runtime:

```rust,ignore
use gloam_commands::prelude::*;

struct State;

#[command(description = "Say hello")]
async fn hello(
    ctx: Context<State>,
    #[description = "Person to greet"]
    #[min_length = 1]
    #[max_length = 64]
    name: String,
    #[description = "Reply privately"] private: Option<bool>,
) -> Result<()> {
    let greeting = format!("Hello, {name}!");

    if private.unwrap_or(false) {
        ctx.reply_ephemeral(greeting).await?;
    } else {
        ctx.reply(greeting).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let framework = Framework::builder(State)
        .commands(commands![hello])
        .max_concurrent_commands(64)
        .build()?;

    framework
        .run(std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN"))
        .await
}
```

### Typed command options

The handler signature is the single source of truth for both the static Discord option schema and runtime extraction. The macro generates both from the same parsed parameter list, so registration metadata and invocation extraction cannot be authored independently.

Supported parameter types are:

- `String` → Discord String;
- `bool` → Boolean;
- `i64` → Integer;
- `f64` → Number;
- Gloamwire `UserId` → User;
- Gloamwire `ChannelId` → Channel;
- Gloamwire `RoleId` → Role;
- Gloamwire `AttachmentId` → Attachment;
- `Option<T>` → optional form of any supported `T`.

Every command option requires `#[description = "..."]`. Required parameters must precede optional `Option<T>` parameters, matching Discord's application-command schema rules.

Supported constraints are:

- `#[min = ...]` and `#[max = ...]` for `i64` and `f64`;
- `#[min_length = ...]` and `#[max_length = ...]` for `String`.

The macro validates Discord's supported ranges, rejects incompatible constraints, rejects unsupported parameter types, enforces the 25-option limit, and reports errors at the relevant source span. Framework parameter attributes are removed from the preserved original Rust function after they are consumed by the macro.

Dispatch parses application-command data once through Gloamwire. `Context<D>` retains that parsed data, and the generated adapter extracts each typed parameter before calling the user's async function. Missing required options and malformed option values surface as framework errors rather than being silently defaulted.

### Interaction responses

Each command context owns a shared acknowledgement state that is also shared by clones of that context. Response transitions are serialized so concurrent handlers cannot accidentally send two initial acknowledgements.

Available helpers:

- `ctx.reply(...)` sends the initial public response, completes a matching public deferral, or creates a public followup after the original response.
- `ctx.reply_ephemeral(...)` provides the same behavior for ephemeral responses.
- `ctx.defer()` and `ctx.defer_ephemeral()` send Discord's deferred channel-message acknowledgement.
- `ctx.edit_response(...)` edits the original interaction response and completes a deferred response.
- `ctx.delete_response()` deletes the original response.
- `ctx.followup(...)` and `ctx.followup_ephemeral(...)` create explicit followup messages after acknowledgement.

A deferral fixes the visibility of the original response. Completing a public deferral with `reply_ephemeral(...)`, or an ephemeral deferral with `reply(...)`, returns a framework error rather than silently changing semantics. State advances only after a successful Gloamwire REST call, so failed acknowledgements remain retryable.

### Dispatch and execution

The managed runtime:

- uses Gloamwire's recommended `ShardManager` configuration;
- requests no Gateway intents because application-command interactions do not require one;
- parses only `INTERACTION_CREATE` application-command payloads through Gloamwire's typed dispatch API;
- routes only chat-input commands registered in the local command registry;
- preserves the receiving `ShardId` in `Context<D>`;
- reserves an execution slot before spawning a handler, so command tasks remain bounded;
- continues polling the Gateway while command handlers execute.

Applications that already own a Gloamwire Gateway loop can call `Framework::dispatch(...)` or `Framework::dispatch_shard(...)` instead. Manual dispatch returns `DispatchOutcome`, including `Ignored`, `Unregistered`, `AtCapacity`, and a `Spawned(CommandTask)` handle.

`#[command]` currently:

- requires an `async fn`;
- requires `Context<D>` as the first parameter;
- requires a `Result<()>` return type;
- requires a slash-command description;
- supports an optional explicit `name = "..."`;
- accepts supported typed slash-command parameters after the context;
- validates command names, descriptions, parameter descriptions, option ordering, option count, supported types, and supported constraints;
- preserves the original Rust function;
- generates the static descriptor and erased extraction/handler adapter used by `commands![...]`.

Command synchronization is not implemented yet. Until Phase 6, Discord application commands must be registered externally for managed Gateway dispatch to receive invocations. Phase 6 will convert the generated descriptors into Gloamwire application-command payloads and bulk-synchronize them with Discord.

## Planned synchronization API

Phase 6 will extend framework configuration with explicit registration policy while reusing the descriptors already generated in Phase 5:

```rust,ignore
let framework = Framework::builder(State)
    .commands(commands![hello])
    .registration(Registration::Global)
    .build()?;
```

Registration remains explicit and deterministic; the framework will use Gloamwire's existing application-command REST APIs rather than adding parallel HTTP behavior.

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
