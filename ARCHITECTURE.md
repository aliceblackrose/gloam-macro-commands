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
  ├── Registration
  └── Arc<Semaphore>             # global command execution slots

Runtime<D>
  ├── Arc<RestClient>
  └── Arc<D>

Context<D>
  ├── Arc<Runtime<D>>
  ├── Arc<Interaction>
  ├── Arc<ApplicationCommandInteractionData>
  ├── registered command name
  ├── optional ShardId
  └── Arc<Mutex<ResponseState>>  # shared acknowledgement state
```

`D` is application-owned shared state. The framework stores it behind `Arc` so command contexts can be owned values and can safely move into asynchronous handler tasks without requiring `D: Clone`.

There is no global runtime singleton. Clones of one `Context<D>` share the same interaction, the same parsed application-command data, and the same response state.

## Command model

A registered command is represented by two pieces:

1. static metadata describing the Discord chat-input command;
2. an erased handler adapter generated from the user's typed async function.

The command macro is responsible for ensuring those two pieces are derived from the same Rust signature. Registration metadata and runtime extraction must never be independently authored copies of the same command schema.

```text
#[command]
async fn inspect(
    ctx: Context<State>,
    #[description = "User to inspect"] user: UserId,
    #[description = "Include extra details"] verbose: Option<bool>,
)
        │
        ├── static command + option metadata
        │
        └── generated extraction adapter
                 │
                 ▼
           user handler
```

## Registration model

Registration is explicit and deterministic. The framework uses an explicit command list rather than linker-based distributed registration.

```rust,ignore
Framework::builder(state)
    .commands(commands![ping, inspect, admin::ban])
    .registration(Registration::Guild(development_guild_id))
    .build()?;
```

`Registration` has three policies:

- `Registration::Guild(GuildId)` bulk-overwrites one guild's command set and is the recommended development workflow because guild updates propagate quickly;
- `Registration::Global` bulk-overwrites the application's global command set;
- `Registration::None` leaves registration externally managed and is the default.

The safe default is deliberate: Discord bulk overwrite replaces the target command set, so constructing or running a framework must not mutate Discord command state unless the application explicitly selects a synchronization target.

Synchronization walks `CommandRegistry<D>` in its existing `BTreeMap` order, converts each generated `CommandDescriptor` and `CommandOptionDescriptor` into Gloamwire's public application-command request models, and calls Gloamwire's existing global or guild bulk-overwrite REST method. The framework does not duplicate command HTTP routes or maintain a second registration schema.

In managed mode, the first typed Discord `READY` event supplies `ReadyApplication.id`. The framework synchronizes exactly once with that application ID before continuing normal managed dispatch. Applications that own their Gateway loop can call `Framework::synchronize_commands(&rest, application_id)` explicitly.

Linker-based registration such as `inventory` or `linkme` is not part of the 0.1 design.

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

The framework reuses Gloamwire's `Interaction`, `GatewayEvent`, `ShardEvent`, and `ShardManager` types directly rather than creating parallel Discord models. `DispatchEvent::typed()` remains Gloamwire's responsibility; the framework only applies command-specific routing after typed decoding.

For a chat-input invocation, dispatch parses `ApplicationCommandInteractionData` once through Gloamwire, uses that value for command routing, and stores it in `Context<D>`. Generated typed-option adapters therefore extract from the already-parsed command data instead of decoding the interaction payload a second time.

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

Manual dispatch surfaces saturation as `DispatchOutcome::AtCapacity`. The managed runtime also refuses to create a task when saturated and immediately continues Gateway polling. A later execution-policy phase can add application-configurable handling without weakening this scheduler invariant.

## Managed runtime

`Framework::run(...)` creates a Gloamwire `RestClient`, starts Gloamwire's recommended shard set through `ShardManager`, and continuously consumes its unified `ShardEvent` stream.

If registration is `Global` or `Guild`, managed mode obtains the application ID from the first Gloamwire `TypedDispatchEvent::Ready` and performs one deterministic synchronization before normal command handling continues. `Registration::None` skips this path entirely.

Slash-command dispatch requests `GatewayIntents::empty()` because Discord application-command interactions do not require Gateway intent subscriptions. Applications that also need guild/message/member event streams should own their Gloamwire loop and use manual framework dispatch with whatever intents their application requires.

Shard identity is copied into `Context<D>` when dispatch originates from `ShardEvent`.

## Context design

`Context<D>` is deliberately slash-command-specific. It exposes the original Gloamwire `Interaction` and parsed `ApplicationCommandInteractionData` rather than hiding Discord data behind duplicate wrappers.

Current accessors include:

```text
ctx.data()
ctx.rest()
ctx.runtime()
ctx.interaction()
ctx.command_data()
ctx.command_name()
ctx.shard_id()
```

Current response helpers include:

```text
ctx.reply(...)
ctx.reply_ephemeral(...)
ctx.defer()
ctx.defer_ephemeral()
ctx.edit_response(...)
ctx.delete_response()
ctx.followup(...)
ctx.followup_ephemeral(...)
```

Planned accessors include:

```text
ctx.guild_id()
ctx.channel_id()
ctx.user()
ctx.member()
ctx.command_path()
```

## Response state

Discord interactions allow one initial acknowledgement. The framework tracks that acknowledgement state per command context and shares it across all clones of that context.

The state machine is:

```text
Pending
  ├── reply ───────────────► Responded
  └── defer(public/private)► Deferred { visibility }

Deferred
  ├── matching reply ──────► Responded       # edits @original
  ├── edit original ───────► Responded
  ├── delete original ─────► Deleted
  └── followup ────────────► Deferred         # original remains deferred

Responded
  ├── reply ───────────────► Responded        # creates followup
  ├── edit original ───────► Responded
  ├── delete original ─────► Deleted
  └── followup ────────────► Responded

Deleted
  └── reply/followup ──────► Deleted          # followup webhook remains usable
```

A Tokio mutex serializes transitions and stays held while the state-changing Gloamwire REST request is in flight. This prevents two cloned contexts from both deciding they own the initial acknowledgement. The state is advanced only after a successful REST call, so a failed initial reply/defer/edit/delete does not corrupt the local transition state.

Deferral visibility is invariant for the original response. A public deferral cannot be completed through an ephemeral reply helper, and an ephemeral deferral cannot be completed through the public reply helper. Those cases return a framework error before sending an invalid or misleading request.

Explicit `followup` calls require an acknowledged interaction. Automatic `reply` calls become followups once the original response is already acknowledged.

## Typed option model

Supported Rust handler parameters define Discord option schemas and extraction behavior together.

Current scalar mappings:

| Rust type | Discord option |
| --- | --- |
| `String` | String |
| `bool` | Boolean |
| `i64` | Integer |
| `f64` | Number |
| `UserId` | User |
| `ChannelId` | Channel |
| `RoleId` | Role |
| `AttachmentId` | Attachment |
| `Option<T>` | Optional form of `T` |

Every option parameter carries `#[description = "..."]`. `String` parameters may also carry `#[min_length = ...]` / `#[max_length = ...]`; `i64` and `f64` parameters may carry `#[min = ...]` / `#[max = ...]`. Required options must precede `Option<T>` parameters.

The macro validates supported types, option count, descriptions, ordering, constraint compatibility, and Discord-supported ranges. Framework-specific parameter attributes are consumed and removed before the original Rust function is emitted, so the original typed function remains callable.

`CommandOptionDescriptor` is the registration-side representation. `CommandOptions` and the `CommandOption` trait are the runtime extraction side. Both are generated or invoked from the same parsed macro parameter list, which prevents registration metadata and extraction behavior from drifting into separately maintained schemas.

Unsupported parameter types fail at macro expansion with a diagnostic attached to the unsupported parameter span. Missing or malformed submitted values fail through explicit runtime option-extraction errors.

## Subcommands

The framework will model Discord's native hierarchy only:

```text
/command
/group subcommand
/group subgroup subcommand
```

It will not invent deeper application-only nesting that Discord cannot register.

## Error boundaries

Framework errors represent framework invariants such as:

- duplicate command registration;
- invalid concurrency configuration;
- invalid interaction payloads;
- invalid option extraction;
- unknown command paths;
- an acknowledgement required before edit/delete/followup;
- a repeated deferral;
- an original response that was already deleted;
- a deferral visibility mismatch;
- failed checks.

Protocol/REST/Gateway errors, including command synchronization failures, remain Gloamwire errors wrapped transparently rather than copied into parallel error models.

## API design rules

1. Prefer public Gloamwire types over wrapper types when no framework-specific invariant exists.
2. Keep macro expansion thin; runtime logic belongs in `gloam-commands`.
3. Keep user state explicit and shared through `Runtime<D>`/`Context<D>`.
4. Do not require message-content or other Gateway intents for slash-command functionality.
5. Keep registration deterministic and reject duplicate command paths early.
6. Generate registration metadata and option extraction from the same source signature.
7. Serialize and expose Discord acknowledgement semantics instead of hiding invalid transitions.
8. Keep managed execution optional; advanced applications must be able to dispatch interactions from their own Gateway loop.
9. Do not duplicate Gloamwire REST, Gateway, sharding, interaction, or Discord model implementations.
10. Never block Gateway polling on command execution capacity and never create unbounded command waiter tasks.
11. Prefix commands are a non-goal, not a deferred feature.
