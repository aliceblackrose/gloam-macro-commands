# Roadmap

Gloam Macro Commands is intentionally a **Discord slash-command-only** framework built on top of Gloamwire. Prefix commands, message-content command parsing, and hybrid prefix/slash abstractions are out of scope.

The roadmap is organized so each phase is independently reviewable and leaves the repository in a coherent state. Later phases should reuse the abstractions established earlier instead of introducing parallel implementations.

## Version 0.1 scope

Version 0.1 targets a complete slash-command foundation:

- runtime and per-command context;
- `#[command]` procedural macro;
- interaction dispatch;
- response helpers and acknowledgement tracking;
- typed slash-command options;
- global/guild command synchronization;
- subcommands and subcommand groups;
- choices and autocomplete;
- checks, execution policy, and hooks;
- examples, documentation, and release hardening.

## Phase 1 — Runtime foundation

**Status:** Complete

Establish the crate boundaries and runtime abstractions before generated code depends on them.

- [x] Create the Cargo workspace.
- [x] Add `gloam-commands` runtime crate.
- [x] Add `gloam-commands-macros` proc-macro crate scaffold.
- [x] Pin the workspace to the current Gloamwire revision.
- [x] Add `Runtime<D>` for shared REST client and application state.
- [x] Add `Context<D>` as the per-execution command context.
- [x] Add static `CommandDescriptor` metadata.
- [x] Add erased `SlashCommand<D>` handler storage.
- [x] Add deterministic `CommandRegistry<D>` lookup.
- [x] Reject duplicate command names.
- [x] Add `Framework<D>` and `FrameworkBuilder<D>`.
- [x] Run formatting, tests, Clippy, and documentation checks in CI.
- [x] Merge the Phase 1 branch after the foundation is green.

**Exit criteria:** the workspace builds cleanly and exposes stable ownership boundaries for runtime state, contexts, command descriptors, and registration.

## Phase 2 — `#[command]` macro

**Status:** Complete

Implement the first procedural macro without adding typed options yet.

- [x] Add `syn`, `quote`, and `proc-macro2` only when macro implementation begins.
- [x] Parse async command functions.
- [x] Require `Context<D>` as the command context parameter.
- [x] Require command handlers to return `Result<()>`.
- [x] Generate static `CommandDescriptor` metadata.
- [x] Generate the erased async handler adapter.
- [x] Preserve the user's original command function.
- [x] Re-export `#[command]` from `gloam-commands`.
- [x] Validate Discord chat-input command names and description lengths at macro expansion time.
- [x] Add compile-fail tests for invalid command signatures.
- [x] Add precise span-based macro diagnostics.
- [x] Add `commands![...]` for explicit deterministic registration.

**Exit criteria:** a zero-option slash command can be declared with `#[command]` and registered without handwritten descriptor or adapter code.

## Phase 3 — Interaction dispatch and managed runtime

**Status:** Complete

Connect Gloamwire's Gateway interaction stream to registered handlers.

- [x] Route `INTERACTION_CREATE` application-command interactions.
- [x] Ignore unrelated Gateway events without consuming application behavior.
- [x] Resolve commands by top-level chat-input command name.
- [x] Construct framework-owned `Context<D>` values.
- [x] Spawn command handlers without blocking Gateway polling.
- [x] Bound concurrent command execution with an explicit semaphore/configuration limit.
- [x] Preserve shard identity in command context when available.
- [x] Add a managed `Framework::run(...)` path.
- [x] Add a manual `Framework::dispatch(...)` path for applications that own their Gateway loop.
- [x] Keep required Gateway intents minimal for slash commands.

**Exit criteria:** `/ping` can execute end-to-end through a real `INTERACTION_CREATE` dispatch while Gateway polling remains responsive.

## Phase 4 — Interaction responses

**Status:** Complete

Add ergonomic response APIs while preserving Discord acknowledgement rules.

- [x] Track interaction acknowledgement state.
- [x] Add `ctx.reply(...)`.
- [x] Add ephemeral replies.
- [x] Add `ctx.defer()` and ephemeral deferral.
- [x] Add original-response editing.
- [x] Add original-response deletion.
- [x] Add followup messages.
- [x] Automatically use followups after an initial response when appropriate.
- [x] Return explicit errors for invalid acknowledgement transitions.
- [x] Add response-state concurrency tests.

**Exit criteria:** handlers can safely respond, defer, edit, delete, and follow up without manually calling raw interaction endpoints.

## Phase 5 — Typed slash-command options

**Status:** Complete

Generate Discord option schemas and runtime extraction from one Rust function signature.

- [x] Add option descriptor metadata.
- [x] Support `String`.
- [x] Support `bool`.
- [x] Support `i64`.
- [x] Support `f64`.
- [x] Support `UserId`.
- [x] Support `ChannelId`.
- [x] Support `RoleId`.
- [x] Support attachments through `AttachmentId`.
- [x] Support `Option<T>` as an optional option.
- [x] Add `#[description = "..."]` parameter metadata.
- [x] Add numeric minimum/maximum constraints.
- [x] Add string length constraints.
- [x] Validate unsupported Rust parameter types at macro expansion time.
- [x] Ensure registration metadata and runtime extraction cannot drift.

**Exit criteria:** typed Rust parameters fully define both the Discord slash-command schema and handler extraction behavior.

## Phase 6 — Command synchronization

**Status:** Complete

Register generated command schemas through Gloamwire's existing application-command REST APIs.

- [x] Add `Registration::Global`.
- [x] Add `Registration::Guild(GuildId)`.
- [x] Add `Registration::None` for externally managed registration.
- [x] Convert command descriptors to Gloamwire create-command payloads.
- [x] Bulk-overwrite global commands.
- [x] Bulk-overwrite guild commands.
- [x] Keep synchronization deterministic.
- [x] Surface registration failures without hiding Gloamwire errors.
- [x] Document guild registration as the recommended development workflow.

**Exit criteria:** the local registry can be synchronized with Discord without a second handwritten command schema.

## Phase 7 — Subcommands and groups

**Status:** Complete

Model Discord's native chat-input hierarchy directly.

- [x] Add `#[group]`.
- [x] Support `/group subcommand`.
- [x] Support Discord-compatible subcommand groups.
- [x] Resolve full command paths during dispatch.
- [x] Expose the resolved command path from `Context<D>`.
- [x] Enforce Discord hierarchy limits at compile time where possible.
- [x] Detect duplicate paths deterministically.

**Exit criteria:** grouped command modules generate valid Discord subcommand trees and dispatch to the correct handler.

## Phase 8 — Choices and typed choice enums

**Status:** Complete

Add static slash-command choices without falling back to untyped strings.

- [x] Add inline choice metadata for supported scalar option types.
- [x] Add `#[derive(CommandChoice)]`.
- [x] Add `#[choice(name = "...")]` variant metadata.
- [x] Generate Discord choice schemas from enums.
- [x] Convert resolved choice values back into typed enum variants.
- [x] Validate duplicate and invalid choice values at compile time where possible.

**Exit criteria:** handlers can receive typed enum choices while Discord receives matching static choice metadata.

## Phase 9 — Autocomplete

**Status:** Complete

Support Discord application-command autocomplete as a first-class interaction path.

- [x] Add `#[autocomplete]` handlers.
- [x] Add `AutocompleteContext<D>` only if autocomplete-specific semantics justify a distinct type.
- [x] Route autocomplete interactions separately from command execution.
- [x] Expose the focused option and current partial value.
- [x] Allow command options to reference autocomplete handlers.
- [x] Validate autocomplete compatibility with the option type.
- [x] Convert framework choices into Discord autocomplete responses.
- [x] Enforce Discord result limits.

**Exit criteria:** an option can dynamically provide Discord autocomplete results without custom interaction routing.

## Phase 10 — Checks and execution policy

Add composable execution policy around slash-command handlers.

- [ ] Add custom checks.
- [ ] Add guild-only checks.
- [ ] Add context/DM restrictions using Discord application-command contexts.
- [ ] Add member-permission checks.
- [ ] Add bot-permission checks where applicable.
- [ ] Add per-command cooldown policy.
- [ ] Add configurable concurrency limits.
- [ ] Define deterministic check ordering.
- [ ] Keep Discord registration permissions separate from runtime checks.

**Exit criteria:** command eligibility and execution limits are composable, testable, and independent from handler business logic.

## Phase 11 — Hooks, observability, and 0.1 hardening

Finish the first release with diagnostics and complete examples.

- [ ] Add before-command hooks.
- [ ] Add after-command hooks.
- [ ] Add centralized command-error handling.
- [ ] Add optional `tracing` integration without installing a subscriber.
- [ ] Ensure interaction tokens and sensitive payload data are never logged.
- [ ] Add basic command example.
- [ ] Add typed-option example.
- [ ] Add grouped-command example.
- [ ] Add autocomplete example.
- [ ] Document manual-dispatch integration with existing Gloamwire loops.
- [ ] Add CI for formatting, tests, Clippy, and rustdoc.
- [ ] Audit public API documentation.
- [ ] Publish the 0.1 release checklist.

**Exit criteria:** the supported slash-command feature set is documented, tested, observable, and ready for a 0.1 release.

## Explicit non-goals

The following are intentionally not planned for this framework:

- prefix commands such as `!ping`;
- message-content command parsing;
- a prefix tokenizer or argument lexer;
- prefix aliases;
- hybrid prefix/slash command descriptors;
- requiring the `MESSAGE_CONTENT` privileged intent for framework operation;
- replacing Gloamwire's Gateway or REST implementations;
- hidden global application state or hidden singleton runtimes.
