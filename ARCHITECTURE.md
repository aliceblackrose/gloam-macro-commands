# Architecture

## Purpose

Gloam Macro Commands is a high-level Discord **chat-input slash-command framework** built on top of Gloamwire.

Gloamwire remains responsible for Discord protocol transport, Gateway events, REST requests, Discord models, sharding, rate limits, and interaction endpoints. This project adds command declaration, registration metadata, dispatch, typed option extraction, response ergonomics, and execution policy.

The framework does not add prefix commands or message-content parsing.

## Crate boundaries

```text
gloam-commands-macros
        │
        │ generates descriptors and adapters
        ▼
gloam-commands
        │
        │ uses public Gloamwire APIs
        ▼
gloamwire
```

### `gloam-commands`

Owns all runtime behavior:

- framework configuration;
- runtime state;
- command context;
- command registry;
- interaction dispatch;
- option extraction;
- response tracking;
- synchronization;
- checks and hooks.

### `gloam-commands-macros`

Owns compile-time transformation only:

- parse command/group attributes;
- validate supported command signatures;
- generate static descriptors;
- generate erased handler adapters;
- generate choice/group metadata.

The proc-macro crate must not perform Discord runtime behavior, own global state, or duplicate Gloamwire HTTP/Gateway logic.

## Core ownership model

```text
Framework<D>
  ├── Arc<D>
  └── CommandRegistry<D>

Runtime<D>
  ├── Arc<RestClient>
  └── Arc<D>

Context<D>
  ├── Arc<Runtime<D>>
  └── per-interaction command state
```

`D` is application-owned shared state. The framework stores it behind `Arc` so command contexts can be owned values and can safely move into asynchronous handler tasks without requiring `D: Clone`.

There is no global runtime singleton.

## Command model

A registered command is represented by two pieces:

1. static metadata describing the Discord chat-input command;
2. an erased handler adapter generated from the user's typed async function.

The command macro is responsible for ensuring those two pieces are derived from the same Rust signature. Registration metadata and runtime extraction must never be independently authored copies of the same command schema.

```text
#[command]
async fn inspect(
    ctx: Context<State>,
    user: UserId,
    verbose: Option<bool>,
)
        │
        ├── static command metadata
        │
        └── generated extraction adapter
                 │
                 ▼
           user handler
```

## Registration model

Registration is explicit and deterministic. The initial design uses an explicit command list rather than linker-based distributed registration.

Target API:

```rust,ignore
Framework::builder(state)
    .commands(commands![ping, inspect, admin::ban])
    .build()?;
```

This keeps command ordering and duplicate detection visible and testable. Linker-based registration such as `inventory` or `linkme` is not part of the 0.1 design.

## Dispatch model

Only Discord application-command interactions are command invocations.

```text
Gloamwire Gateway event
        │
        ▼
INTERACTION_CREATE
        │
        ├── application command ──► command registry ──► handler
        │
        └── autocomplete ─────────► autocomplete registry/handler
```

Unrelated Gateway events pass through or are ignored by the managed framework path. The manual dispatch API will allow applications to keep owning their existing Gloamwire event loop.

## Concurrency model

Gateway polling must not wait for command business logic. Once an interaction has been validated and resolved, handler execution is spawned separately and bounded by an explicit concurrency limit.

The framework must not create unbounded command tasks.

```text
Gateway polling
    │
    ├── resolve command
    │      └── acquire execution permit
    │             └── spawn handler
    │
    └── continue polling immediately
```

## Context design

`Context<D>` is deliberately slash-command-specific. It should expose Discord interaction data and response helpers directly instead of pretending to represent multiple invocation mechanisms.

Planned accessors include:

```text
ctx.data()
ctx.rest()
ctx.interaction()
ctx.guild_id()
ctx.channel_id()
ctx.user()
ctx.member()
ctx.command_name()
ctx.command_path()
ctx.shard_id()
```

Planned response helpers include:

```text
ctx.reply(...)
ctx.reply_ephemeral(...)
ctx.defer()
ctx.defer_ephemeral()
ctx.edit_response(...)
ctx.delete_response()
ctx.followup(...)
```

## Response state

Discord interactions have acknowledgement constraints. The framework will track the acknowledgement state per interaction so handlers do not need to manually decide between initial callbacks and followups.

Conceptually:

```text
Pending
  ├── reply ─────► Responded
  └── defer ─────► Deferred

Responded/Deferred
  ├── edit original
  └── followup
```

Invalid transitions must return framework errors rather than silently issuing incorrect Discord requests.

## Typed option model

Supported Rust handler parameters define Discord option schemas and extraction behavior together.

Initial scalar mappings:

| Rust type | Discord option |
| --- | --- |
| `String` | String |
| `bool` | Boolean |
| `i64` | Integer |
| `f64` | Number |
| `UserId` | User |
| `ChannelId` | Channel |
| `RoleId` | Role |
| attachment type/ID | Attachment |
| `Option<T>` | Optional form of `T` |

Unsupported parameter types should fail at macro expansion with a diagnostic attached to the unsupported parameter span.

## Subcommands

The framework will model Discord's native hierarchy only:

```text
/command
/group subcommand
/group subgroup subcommand
```

It will not invent deeper application-only nesting that Discord cannot register.

## Error boundaries

Framework errors should represent framework invariants such as:

- duplicate command registration;
- invalid option extraction;
- unknown command paths;
- invalid interaction response transitions;
- failed checks.

Protocol/REST/Gateway errors should remain Gloamwire errors wrapped transparently rather than copied into parallel error models.

## API design rules

1. Prefer public Gloamwire types over wrapper types when no framework-specific invariant exists.
2. Keep macro expansion thin; runtime logic belongs in `gloam-commands`.
3. Keep user state explicit and shared through `Runtime<D>`/`Context<D>`.
4. Do not require message-content intents for slash-command functionality.
5. Keep registration deterministic and reject duplicate command paths early.
6. Generate registration metadata and option extraction from the same source signature.
7. Do not hide Discord acknowledgement state transitions.
8. Keep managed execution optional; advanced applications must be able to dispatch interactions from their own Gateway loop.
9. Do not duplicate Gloamwire REST, Gateway, sharding, or Discord model implementations.
10. Prefix commands are a non-goal, not a deferred feature.
