# Architecture

## Purpose

Gloam Macro Commands is a high-level Discord **chat-input slash-command framework** built on top of Gloamwire.

Gloamwire remains responsible for Discord protocol transport, Gateway events, REST requests, Discord models, sharding, rate limits, and interaction endpoints. This project adds command declaration, registration metadata, dispatch, typed option extraction, static choices, dynamic autocomplete, response ergonomics, and execution policy.

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
- command and autocomplete contexts;
- command registry;
- interaction dispatch;
- option extraction;
- choice extraction and autocomplete response validation;
- response tracking;
- synchronization;
- checks and hooks.

### `gloam-commands-macros`

Owns compile-time transformation only:

- parse command/group/autocomplete attributes;
- parse inline and enum choice metadata;
- validate supported command and autocomplete signatures;
- generate static descriptors;
- generate erased command and autocomplete handler adapters;
- generate choice/group/autocomplete metadata.

The proc-macro crate must not perform Discord runtime behavior, own global state, or duplicate Gloamwire HTTP/Gateway logic.

## Core ownership model

```text
Framework<D>
  ├── Arc<D>
  ├── CommandRegistry<D>
  ├── Registration
  └── Arc<Semaphore>             # shared command/autocomplete execution slots

Runtime<D>
  ├── Arc<RestClient>
  └── Arc<D>

Context<D>
  ├── Arc<Runtime<D>>
  ├── Arc<Interaction>
  ├── Arc<ApplicationCommandInteractionData>
  ├── resolved static command path
  ├── resolved leaf option scope
  ├── optional ShardId
  └── Arc<Mutex<ResponseState>>  # shared acknowledgement state

AutocompleteContext<D>
  ├── Arc<Runtime<D>>
  ├── Arc<Interaction>
  ├── Arc<ApplicationCommandInteractionData>
  ├── resolved static command path
  ├── resolved leaf option scope
  ├── focused option index
  └── optional ShardId
```

`D` is application-owned shared state. The framework stores it behind `Arc` so command and autocomplete contexts can be owned values and can safely move into asynchronous handler tasks without requiring `D: Clone`.

There is no global runtime singleton. Clones of one `Context<D>` share the same interaction, the same parsed application-command data, the same resolved command path and leaf option scope, and the same response state. `AutocompleteContext<D>` shares the same runtime/data ownership model but intentionally has no command-response acknowledgement state because Discord autocomplete uses a dedicated callback response.

## Command model

A registered command is represented by two pieces:

1. static metadata describing the Discord chat-input command;
2. an erased handler adapter generated from the user's typed async function.

Leaf commands may additionally carry generated autocomplete handler associations keyed by option name. The command macro derives the option's `autocomplete` registration flag and its handler association from the same parameter attribute, so registration and runtime routing cannot drift into separate declarations.

The command macro is responsible for ensuring command metadata and typed extraction are derived from the same Rust signature. Registration metadata and runtime extraction must never be independently authored copies of the same command schema.

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

`SlashCommand<D>` is either a handler-bearing leaf or a group node containing validated child commands. Group nodes have no handler or scalar options. Registry validation rejects empty groups, duplicate sibling paths, scalar options on group nodes, and hierarchy deeper than Discord supports. For leaf commands, registry validation also enforces autocomplete compatibility, static-choice exclusivity, and exact option-to-handler associations.

`#[group]` applies this tree model to inline Rust modules. Direct `#[command]` functions become subcommands, and one nested `#[group]` level becomes a Discord subcommand group.

## Registration model

Registration is explicit and deterministic. The framework uses an explicit command list rather than linker-based distributed registration.

```rust,ignore
Framework::builder(state)
    .commands(commands![ping, inspect, admin])
    .registration(Registration::Guild(development_guild_id))
    .build()?;
```

`Registration` has three policies:

- `Registration::Guild(GuildId)` bulk-overwrites one guild's command set and is the recommended development workflow because guild updates propagate quickly;
- `Registration::Global` bulk-overwrites the application's global command set;
- `Registration::None` leaves registration externally managed and is the default.

The safe default is deliberate: Discord bulk overwrite replaces the target command set, so constructing or running a framework must not mutate Discord command state unless the application explicitly selects a synchronization target.

Synchronization walks `CommandRegistry<D>` in its existing `BTreeMap` order and converts each validated `SlashCommand<D>` tree into Gloamwire's public application-command request models. Leaf descriptors emit scalar options, direct group children emit Discord `SUB_COMMAND` options, and nested group nodes emit `SUB_COMMAND_GROUP` options containing subcommand leaves. Scalar option descriptors also carry bounds, string lengths, static choice descriptors, and autocomplete flags; those are converted directly into Gloamwire's existing application-command option and choice models at synchronization time. The framework does not duplicate command HTTP routes or maintain a second registration schema.

In managed mode, the first typed Discord `READY` event supplies `ReadyApplication.id`. The framework synchronizes exactly once with that application ID before continuing normal managed dispatch. Applications that own their Gateway loop can call `Framework::synchronize_commands(&rest, application_id)` explicitly.

Linker-based registration such as `inventory` or `linkme` is not part of the 0.1 design.

## Dispatch model

Discord chat-input application-command executions and application-command autocomplete interactions share the registered command tree but have distinct handler paths.

```text
Gloamwire GatewayEvent
        │
        ▼
INTERACTION_CREATE
        │
        ├── chat-input application command ──► command registry ──► path resolver ──► command handler
        │
        ├── chat-input autocomplete ─────────► command registry ──► path resolver ──► focused option
        │                                                                       │
        │                                                                       ▼
        │                                                              autocomplete handler
        │                                                                       │
        │                                                                       ▼
        │                                                              Gloamwire callback
        └── other interaction/application-command type ────────────────────────► ignored
```

The framework reuses Gloamwire's `Interaction`, `GatewayEvent`, `ShardEvent`, and `ShardManager` types directly rather than creating parallel Discord models. `DispatchEvent::typed()` remains Gloamwire's responsibility; the framework only applies command-specific routing after typed decoding.

For a chat-input invocation, dispatch parses `ApplicationCommandInteractionData` once through Gloamwire, resolves the submitted native subcommand/subcommand-group branch against the registered `SlashCommand<D>` tree, and stores the full static path plus selected leaf option scope in `Context<D>`. Generated typed-option adapters therefore extract only from that resolved leaf scope instead of decoding the interaction payload a second time.

Autocomplete dispatch uses the same typed interaction decoding and command-path resolver. After resolving the leaf, it requires exactly one submitted leaf option with `focused == true`, verifies that the registered option has autocomplete enabled, resolves that option's generated handler, and builds `AutocompleteContext<D>`. The handler's dynamic choices are validated and converted directly into Gloamwire's application-command autocomplete callback model.

Malformed or stale nested paths return `UnknownCommandPath` instead of being routed to a different handler. A group level requires exactly one submitted branch, matching Discord's invocation shape. Missing/multiple focused options and unknown autocomplete option associations fail explicitly instead of falling through to normal command execution.

Applications may choose either execution path:

- `Framework::run(...)` owns the managed `ShardManager` event loop;
- `Framework::dispatch(...)` and `Framework::dispatch_shard(...)` let an application keep ownership of its existing Gloamwire Gateway loop.

Unrelated Gateway events are ignored by framework dispatch and remain available to applications that own the outer event loop.

## Concurrency model

Gateway polling must not wait for command or autocomplete business logic, and the framework must not create unbounded handler tasks.

The execution slot is therefore reserved **before** a command or autocomplete task is spawned. Reservation is non-blocking. If no slot is available, no task is created.

```text
Gateway polling
    │
    ├── resolve command/autocomplete handler
    │      │
    │      └── try reserve execution slot
    │             ├── available ──► spawn handler holding permit
    │             └── full ───────► AtCapacity; do not spawn
    │
    └── continue polling immediately
```

`FrameworkBuilder::max_concurrent_commands(...)` controls the number of framework-owned command and autocomplete handler tasks. The default is finite. A zero limit is rejected at build time.

Manual dispatch surfaces saturation as `DispatchOutcome::AtCapacity`. The managed runtime also refuses to create a task when saturated and immediately continues Gateway polling. A later execution-policy phase can add application-configurable handling without weakening this scheduler invariant.

## Managed runtime

`Framework::run(...)` creates a Gloamwire `RestClient`, starts Gloamwire's recommended shard set through `ShardManager`, and continuously consumes its unified `ShardEvent` stream.

If registration is `Global` or `Guild`, managed mode obtains the application ID from the first Gloamwire `TypedDispatchEvent::Ready` and performs one deterministic synchronization before normal interaction handling continues. `Registration::None` skips this path entirely.

Slash-command and autocomplete dispatch requests `GatewayIntents::empty()` because Discord application-command interactions do not require Gateway intent subscriptions. Applications that also need guild/message/member event streams should own their Gloamwire loop and use manual framework dispatch with whatever intents their application requires.

Shard identity is copied into `Context<D>` or `AutocompleteContext<D>` when dispatch originates from `ShardEvent`.

## Context design

`Context<D>` is deliberately slash-command-execution-specific. It exposes the original Gloamwire `Interaction` and parsed `ApplicationCommandInteractionData` rather than hiding Discord data behind duplicate wrappers.

Current accessors include:

```text
ctx.data()
ctx.rest()
ctx.runtime()
ctx.interaction()
ctx.command_data()
ctx.command_options()
ctx.command_name()
ctx.command_path()
ctx.shard_id()
```

`command_data()` exposes the top-level parsed application command. `command_options()` exposes only the resolved leaf option scope used by generated typed extraction. `command_path()` exposes the static registered path, for example `admin config set` as three path components.

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

`AutocompleteContext<D>` exists because autocomplete has different acknowledgement semantics while still needing the same shared application state and resolved command information. Its accessors include:

```text
ctx.data()
ctx.rest()
ctx.runtime()
ctx.interaction()
ctx.command_data()
ctx.command_options()
ctx.command_name()
ctx.command_path()
ctx.focused_name()
ctx.focused_value()
ctx.shard_id()
```

Normal reply/defer/edit/followup helpers are intentionally absent from `AutocompleteContext<D>`. The framework owns the autocomplete callback after the handler returns its `Vec<AutocompleteChoice>`.

Planned convenience accessors for command contexts include:

```text
ctx.guild_id()
ctx.channel_id()
ctx.user()
ctx.member()
```

## Response state

Discord command interactions allow one initial acknowledgement. The framework tracks that acknowledgement state per command context and shares it across all clones of that context.

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

Autocomplete interactions do not use this state machine; they produce the dedicated Discord autocomplete callback through the framework-owned handler return path.

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

String, Integer, and Number parameters may instead attach a generated autocomplete handler through `#[autocomplete = handler_path]`. Autocomplete can be used with optional forms of those scalar types, but it cannot coexist with static choices on the same Discord option.

The macro validates supported types, option count, descriptions, ordering, constraint compatibility, choice/autocomplete compatibility, and Discord-supported ranges. Framework-specific parameter attributes are consumed and removed before the original Rust function is emitted, so the original typed function remains callable.

`CommandOptionDescriptor` is the registration-side representation. `CommandOptions` and the `CommandOption` trait are the runtime extraction side. Both are generated or invoked from the same parsed macro parameter list, which prevents registration metadata and extraction behavior from drifting into separately maintained schemas. The same option descriptor also carries the autocomplete registration flag used to validate its generated handler association.

Unsupported parameter types fail at macro expansion with a diagnostic attached to the unsupported parameter span. Missing or malformed submitted values fail through explicit runtime option-extraction errors.

## Choice model

Static choices extend the existing typed option model rather than introducing a second invocation path.

`CommandOptionDescriptor` owns a borrowed slice of `CommandChoiceDescriptor` values. Each choice stores a display name and one static String, Integer, or Number value. Registration allocates Gloamwire's owned choice request values only when converting the descriptor tree for synchronization.

Built-in `String`, `i64`, and `f64` parameters may declare repeated inline `#[choice(name = "...", value = ...)]` attributes. The command macro validates that every value matches the parameter kind, that names and values are unique, and that Discord count/length/numeric limits are respected.

Typed choice enums derive `CommandChoice`. The derive generates both:

1. static `CommandChoiceDescriptor` metadata used by registration;
2. a `CommandOption` extraction implementation that maps the submitted scalar value back into the enum variant.

A typed enum parameter is marked with a bare `#[choice]` so the command macro intentionally accepts the otherwise unsupported user-defined type and sources its option kind and static metadata from `CommandChoice`. `Option<T>` preserves the same choice schema while making the submitted option optional.

Choice enums are unit-variant enums and support String, Integer, or Number values. Variant display names are explicit. Omitted values are allowed only for String choice enums and use the Rust variant identifier as the submitted string value. Mixed String/numeric sets, duplicate names or values, unsupported fields, excessive choice counts, and out-of-range values are rejected at macro expansion.

Discord normally constrains submitted values through the registered choices, but dispatch still treats the incoming interaction as untrusted input. If a stale or malformed interaction submits a scalar value that is not represented by the typed enum, extraction returns `InvalidChoice` before the user handler runs.

Static choices and autocomplete are mutually exclusive on one Discord option, matching Discord's application-command schema.

## Autocomplete model

Autocomplete extends the existing command tree rather than introducing a parallel registry. `#[autocomplete]` preserves a typed async function and generates an erased adapter. A command parameter references that handler with `#[autocomplete = handler_path]`; the command macro rewrites local or module-qualified Rust paths to the generated adapter at compile time.

Autocomplete handlers accept exactly one `AutocompleteContext<D>` and return `Result<Vec<AutocompleteChoice>>`. Dynamic values may be String, Integer, or Number, matching the focused option kind. At dispatch time the context exposes the full static command path, resolved leaf options, focused option name, and Gloamwire's current partial option value.

The framework treats returned choices as untrusted application output. Before any HTTP request it enforces Discord's 25-result limit, 1–100-character choice names, String value maximum length, safe Integer range, finite Number range, and value-kind compatibility with the registered focused option. Empty String choice values are valid; only the documented maximum length is enforced.

After validation, choices are converted into Gloamwire's `ApplicationCommandOptionChoice` values and sent through Gloamwire's existing `create_interaction_response` API using the application-command autocomplete callback type. No second REST route or Discord response model is maintained by the framework.

## Subcommands

The framework models Discord's native hierarchy only:

```text
/command
/group subcommand
/group subgroup subcommand
```

`#[group(description = "...")]` applies to an inline module. Direct `#[command]` functions become Discord subcommands. One direct nested `#[group]` becomes a Discord subcommand group whose direct `#[command]` functions are its nested subcommands.

The macro validates group names and descriptions using the same Discord rules as commands and rejects a third group level at expansion time. Runtime registry validation remains authoritative for manually constructed trees, including duplicate paths, empty groups, scalar options on group nodes, and excessive depth.

The command registry stores only top-level names. Full nested paths resolve within the selected top-level `SlashCommand<D>` tree, keeping a single deterministic registry rather than introducing a parallel subcommand or autocomplete registry.

The framework does not invent deeper application-only nesting that Discord cannot register.

## Error boundaries

Framework errors represent framework invariants such as:

- duplicate command registration;
- invalid concurrency configuration;
- invalid interaction payloads;
- invalid option extraction;
- invalid typed choice extraction;
- unknown command paths;
- invalid autocomplete configuration, focus, or handler output;
- an acknowledgement required before edit/delete/followup;
- a repeated deferral;
- an original response that was already deleted;
- a deferral visibility mismatch;
- failed checks.

Protocol/REST/Gateway errors, including command synchronization and autocomplete callback failures, remain Gloamwire errors wrapped transparently rather than copied into parallel error models.

## API design rules

1. Prefer public Gloamwire types over wrapper types when no framework-specific invariant exists.
2. Keep macro expansion thin; runtime logic belongs in `gloam-commands`.
3. Keep user state explicit and shared through `Runtime<D>` and framework-owned contexts.
4. Do not require message-content or other Gateway intents for slash-command functionality.
5. Keep registration deterministic and reject duplicate command paths early.
6. Generate registration metadata, choices, autocomplete associations, and option extraction from the same command/type declarations.
7. Serialize and expose Discord command acknowledgement semantics instead of hiding invalid transitions; keep autocomplete on its dedicated callback path.
8. Keep managed execution optional; advanced applications must be able to dispatch interactions from their own Gateway loop.
9. Do not duplicate Gloamwire REST, Gateway, sharding, interaction, or Discord model implementations.
10. Never block Gateway polling on command/autocomplete execution capacity and never create unbounded waiter tasks.
11. Prefix commands are a non-goal, not a deferred feature.
