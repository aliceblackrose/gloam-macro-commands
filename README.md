# Gloam Macro Commands

A focused Discord **slash-command framework** for [Gloamwire](https://github.com/aliceblackrose/gloamwire), written for Rust 1.98 and Edition 2024.

> **Current status:** Phase 8 — choices and typed choice enums.

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
- Model Discord-native subcommands and subcommand groups without a parallel nested registry.
- Keep static choice registration and typed choice extraction derived from one enum definition.

## Workspace

```text
crates/
├── gloam-commands/         # runtime, context, registry, dispatch, responses, options
└── gloam-commands-macros/  # procedural macro generation only
```

The proc-macro crate contains no Discord runtime behavior. Generated code targets public runtime abstractions from `gloam-commands`, which in turn uses Gloamwire's Gateway, REST, sharding, interaction, and model APIs.

## Current API

Typed slash commands can be declared with `#[command]`, composed into Discord-native trees with `#[group]`, synchronized with Discord, respond through `Context<D>`, and execute through the managed Gloamwire shard runtime:

```rust,ignore
use gloam_commands::prelude::*;
use gloamwire::model::GuildId;

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
        .registration(Registration::Guild(GuildId::new(123456789012345678)))
        .max_concurrent_commands(64)
        .build()?;

    framework
        .run(std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN"))
        .await
}
```

Use guild registration while developing so command updates propagate quickly. Switch to `Registration::Global` when the command set should be published globally. `Registration::None` is the safe default and leaves Discord registration externally managed.

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

### Choices and typed choice enums

Static Discord choices use the same generated option descriptor that powers registration and runtime extraction. Built-in String, Integer, and Number parameters can declare inline choices directly:

```rust,ignore
#[command(description = "Choose output format")]
async fn format(
    ctx: Context<State>,
    #[description = "Output format"]
    #[choice(name = "Text", value = "text")]
    #[choice(name = "JSON", value = "json")]
    format: String,
) -> Result<()> {
    ctx.reply(format!("Selected {format}")).await?;
    Ok(())
}
```

For a typed handler parameter, derive `CommandChoice` on a unit-variant enum and mark the parameter with a bare `#[choice]`:

```rust,ignore
use gloam_commands::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, CommandChoice)]
enum Mode {
    #[choice(name = "Fast", value = "fast")]
    Fast,
    #[choice(name = "Safe", value = "safe")]
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, CommandChoice)]
enum Level {
    #[choice(name = "Low", value = 1)]
    Low,
    #[choice(name = "High", value = 2)]
    High,
}

#[command(description = "Configure execution")]
async fn configure(
    ctx: Context<State>,
    #[description = "Execution mode"]
    #[choice]
    mode: Mode,
    #[description = "Optional level"]
    #[choice]
    level: Option<Level>,
) -> Result<()> {
    ctx.reply(format!("Configured {mode:?} at {level:?}")).await?;
    Ok(())
}
```

Typed choice enums support Discord String, Integer, and Number choices. Enum values must use one compatible scalar kind; an integer value may participate in a Number enum when another variant uses a floating-point value. Variant display names are required. If a variant omits `value`, the enum is a String choice enum and the Rust variant identifier is used as the submitted value.

The derive rejects non-unit variants, duplicate names or values, mixed String/numeric values, out-of-range numeric values, choice names outside Discord's length limits, string values longer than Discord allows, and more than 25 variants. Typed extraction returns the enum variant and reports `InvalidChoice` if a malformed or stale interaction submits an unregistered value.

Inline choices are supported only for built-in `String`, `i64`, and `f64` options and are likewise validated for count, duplicate names/values, value kind, and Discord ranges. Choice metadata is converted directly into Gloamwire's existing application-command choice model during synchronization; there is no parallel HTTP schema.

### Subcommands and groups

`#[group]` applies to an inline module and generates a Discord-native command tree. Direct `#[command]` functions become subcommands. One nested `#[group]` level becomes a Discord subcommand group containing its direct command leaves:

```rust,ignore
use gloam_commands::prelude::*;

struct State;

#[group(description = "Administration commands")]
mod admin {
    use super::*;

    #[command(description = "Ban a user")]
    async fn ban(
        ctx: Context<State>,
        #[description = "User ID"] user: gloamwire::model::UserId,
    ) -> Result<()> {
        ctx.reply(format!("Would ban {user}")).await?;
        Ok(())
    }

    #[group(description = "Configuration commands")]
    mod config {
        use super::*;

        #[command(description = "Set a value")]
        async fn set(
            ctx: Context<State>,
            #[description = "Configured value"] count: i64,
        ) -> Result<()> {
            let path = ctx.command_path();
            debug_assert_eq!(path, ["admin", "config", "set"]);
            ctx.reply(format!("Configured {count}")).await?;
            Ok(())
        }
    }
}

let framework = Framework::builder(State)
    .commands(commands![admin])
    .build()?;
```

The supported hierarchy is exactly Discord's native shape: a top-level command may contain direct subcommands or subcommand groups, and a subcommand group contains subcommands. Deeper `#[group]` nesting is rejected at macro expansion time. Runtime registry validation also rejects empty groups, duplicate nested paths, scalar options on group nodes, and trees deeper than Discord supports when commands are constructed manually.

Dispatch resolves the full submitted path before scheduling a handler. `ctx.command_path()` exposes that static path, while generated typed extraction reads only the resolved leaf's scalar options. Malformed or stale nested paths return `UnknownCommandPath` instead of falling through to another handler.

### Command synchronization

Registration is explicit because Discord bulk overwrite replaces the target command set. `FrameworkBuilder` therefore defaults to `Registration::None`; simply starting a framework never mutates Discord command registration unless a target is selected.

Available policies are:

- `Registration::Guild(guild_id)` — recommended during development because guild command changes propagate quickly;
- `Registration::Global` — bulk-overwrites the application's global command set;
- `Registration::None` — performs no registration HTTP requests and leaves command management external.

Generated command trees are converted directly into Gloamwire's application-command request models. Top-level leaves emit scalar options; direct children of a group emit Discord `SUB_COMMAND` options; nested groups emit `SUB_COMMAND_GROUP` options containing their command leaves. Scalar option metadata includes requiredness, bounds, string lengths, and static choices from the same generated descriptors used by handler extraction. Ordering stays deterministic because synchronization walks the framework's `BTreeMap`-backed registry and preserves each validated child vector's order.

In managed mode, `Framework::run(...)` waits for the first Discord `READY` dispatch, reads the application ID from Gloamwire's typed `ReadyEvent`, and synchronizes once before continuing normal command handling. Applications that own their Gateway loop can synchronize explicitly:

```rust,ignore
framework
    .synchronize_commands(&rest, application_id)
    .await?;
```

Both global and guild synchronization reuse Gloamwire's existing bulk-overwrite REST methods. Gloamwire failures propagate through the framework's transparent Gloamwire error variant instead of being translated into a parallel HTTP error model.

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
- optionally synchronizes the registry once from the first typed `READY` event;
- parses only `INTERACTION_CREATE` application-command payloads through Gloamwire's typed dispatch API;
- routes only chat-input commands registered in the local command registry;
- resolves native subcommand/subcommand-group branches before handler scheduling;
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
- supports inline static choices on String, Integer, and Number options;
- supports typed `CommandChoice` enum parameters through a bare `#[choice]` marker;
- validates command names, descriptions, parameter descriptions, option ordering, option count, supported types, constraints, and static choices;
- preserves the original Rust function;
- generates the static descriptor and erased extraction/handler adapter used by `commands![...]`.

`#[group]` currently:

- requires an inline module;
- requires a slash-command description and supports an optional explicit name;
- accepts direct `#[command]` and `#[group]` children;
- requires at least one command leaf;
- reuses command name/description validation;
- rejects hierarchy deeper than Discord supports;
- generates the group descriptor/factory consumed by `commands![...]`.

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
