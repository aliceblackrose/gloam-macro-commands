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
  ├── CommandRegistry<D>
  └── Arc<Semaphore>             # global command execution slots

Runtime<D>
  ├── Arc<RestClient>
  └── Arc<D>

Context<D>
  ├── Arc<Runtime<D>>
  ├── Arc<Interaction>
  ├── registered command name
  └── optional ShardId
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

Only Discord chat-input application-command interactions are command invocations.

```text
Gloamwire GatewayEvent
        │
        ▼
INTERACTION_CREATE
        │
        ├── chat-input application command ──► command registry ──► execution scheduler
        │
        ├── other application-command type ──► ignored
        ├── autocomplete ─────────────────────► later autocomplete path
        └── other interaction type ──────────► ignored
```

The framework reuses Gloamwire's `Interaction`, `GatewayEvent`, `ShardEvent`, and `ShardManager` types directly rather than creating parallel Discord models.

Applications may choose either execution path:

- `Framework::run(...)` owns the managed `ShardManager` event loop;
- `Framework::dispatch(...)` and `Framework::dispatch_shard(...)` let an application keep ownership of its existing Gloamwire Gateway loop.

Unrelated Gateway events are ignored by framework dispatch and remain available to applications that own the outer event loop.

## Concurrency model

Gateway polling must not wait for command business logic, and the framework must not create unbounded command tasks.

The execution slot is therefore reserved **before** a command task is spawned. Reservation is non-blocking. If no slot is available, no task is created.

```text
Gateway polling
    │
    ├── resolve command
    │      │
    │      └── try reserve execution slot
    │             ├── available ──► spawn handler holding permit
    │             └── full ───────► AtCapacity; do not spawn
    │
    └── continue polling immediately
```

`FrameworkBuilder::max_concurrent_commands(...)` controls the number of framework-owned handler tasks. The default is finite. A zero limit is rejected at build time.

Manual dispatch surfaces saturation as `DispatchOutcome::AtCapacity`. The managed runtime also refuses to create a task when saturated and immediately continues Gateway polling. A later response-policy phase can decide how saturated interactions should be acknowledged to Discord without weakening this scheduler invariant.

## Managed runtime

`Framework::run(...)` creates a Gloamwire `RestClient`, starts Gloamwire's recommended shard set through `ShardManager`, and continuously consumes its unified `ShardEvent` stream.

Slash-command dispatch requests `GatewayIntents::empty()` because Discord application-command interactions do not require Gateway intent subscriptions. Applications that also need guild/message/member event streams should own their Gloamwire loop and use manual framework dispatch with whatever intents their application requires.

Shard identity is copied into `Context<D>` when dispatch originates from `ShardEvent`.

## Context design

`Context<D>` is deliberately slash-command-specific. It exposes the original Gloamwire `Interaction` rather than hiding Discord data behind a duplicate wrapper.

Current accessors include:

```text
ctx.data()
ctx.rest()
ctx.runtime()
ctx.interaction()
ctx.command_name()
ctx.shard_id()
```

Planned accessors include:

```text
ctx.guild_id()
ctx.channel_id()
ctx.user()
ctx.member()
ctx.command_path()
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
- invalid concurrency configuration;
- invalid interaction payloads;
- invalid option extraction;
- unknown command paths;
- invalid interaction response transitions;
- failed checks.

Protocol/REST/Gateway errors remain Gloamwire errors wrapped transparently rather than copied into parallel error models.

## API design rules

1. Prefer public Gloamwire types over wrapper types when no framework-specific invariant exists.
2. Keep macro expansion thin; runtime logic belongs in `gloam-commands`.
3. Keep user state explicit and shared through `Runtime<D>`/`Context<D>`.
4. Do not require message-content or other Gateway intents for slash-command functionality.
5. Keep registration deterministic and reject duplicate command paths early.
6. Generate registration metadata and option extraction from the same source signature.
7. Do not hide Discord acknowledgement state transitions.
8. Keep managed execution optional; advanced applications must be able to dispatch interactions from their own Gateway loop.
9. Do not duplicate Gloamwire REST, Gateway, sharding, or Discord model implementations.
10. Never block Gateway polling on command execution capacity and never create unbounded command waiter tasks.
11. Prefix commands are a non-goal, not a deferred feature.
