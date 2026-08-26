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

**Status:** In progress

Implement the first procedural macro without adding typed options yet.

- [x] Add `syn`, `quote`, and `proc-macro2` only when macro implementation begins.
- [x] Parse async command functions.
- [x] Require `Context<D>` as the command context parameter.
- [x] Generate static `CommandDescriptor` metadata.
- [x] Generate the erased async handler adapter.
- [x] Preserve the user's original command function.
- [x] Re-export `#[command]` from `gloam-commands`.
- [x] Validate Discord chat-input command names and description lengths at macro expansion time.
- [ ] Add compile-fail tests for invalid command signatures.
- [ ] Add precise span-based macro diagnostics.
- [x] Add `commands![...]` for explicit deterministic registration.

**Exit criteria:** a zero-option slash command can be declared with `#[command]` and registered without handwritten descriptor or adapter code.

## Phase 3 — Interaction dispatch and managed runtime

Connect Gloamwire's Gateway interaction stream to registered handlers.

- [ ] Route `INTERACTION_CREATE` application-command interactions.
- [ ] Ignore unrelated Gateway events without consuming application behavior.
- [ ] Resolve commands by top-level chat-input command name.
- [ ] Construct framework-owned `Context<D>` values.
- [ ] Spawn command handlers without blocking Gateway polling.
- [ ] Bound concurrent command execution with an explicit semaphore/configuration limit.
- [ ] Preserve shard identity in command context when available.
- [ ] Add a managed `Framework::run(...)` path.
- [ ] Add a manual `Framework::dispatch(...)` path for applications that own their Gateway loop.
- [ ] Keep required Gateway intents minimal for slash commands.

**Exit criteria:** `/ping` can execute end-to-end through a real `INTERACTION_CREATE` dispatch while Gateway polling remains responsive.

## Phase 4 — Interaction responses

Add ergonomic response APIs while preserving Discord acknowledgement rules.

- [ ] Track interaction acknowledgement state.
- [ ] Add `ctx.reply(...)`.
- [ ] Add ephemeral replies.
- [ ] Add `ctx.defer()` and ephemeral deferral.
- [ ] Add original-response editing.
- [ ] Add original-response deletion.
- [ ] Add followup messages.
- [ ] Automatically use followups after an initial response when appropriate.
- [ ] Return explicit errors for invalid acknowledgement transitions.
- [ ] Add response-state concurrency tests.

**Exit criteria:** handlers can safely respond, defer, edit, delete, and follow up without manually calling raw interaction endpoints.

## Phase 5 — Typed slash-command options

Generate Discord option schemas and runtime extraction from one Rust function signature.

- [ ] Add option descriptor metadata.
- [ ] Support `String`.
- [ ] Support `bool`.
- [ ] Support `i64`.
- [ ] Support `f64`.
- [ ] Support `UserId`.
- [ ] Support `ChannelId`.
- [ ] Support `RoleId`.
- [ ] Support attachments.
- [ ] Support `Option<T>` as an optional option.
- [ ] Add `#[description = "..."]` parameter metadata.
- [ ] Add numeric minimum/maximum constraints.
- [ ] Add string length constraints.
- [ ] Validate unsupported Rust parameter types at macro expansion time.
- [ ] Ensure registration metadata and runtime extraction cannot drift.

**Exit criteria:** typed Rust parameters fully define both the Discord slash-command schema and handler extraction behavior.

## Phase 6 — Command synchronization

Register generated command schemas through Gloamwire's existing application-command REST APIs.

- [ ] Add `Registration::Global`.
- [ ] Add `Registration::Guild(GuildId)`.
- [ ] Add `Registration::None` for externally managed registration.
- [ ] Convert command descriptors to Gloamwire create-command payloads.
- [ ] Bulk-overwrite global commands.
- [ ] Bulk-overwrite guild commands.
- [ ] Keep synchronization deterministic.
- [ ] Surface registration failures without hiding Gloamwire errors.
- [ ] Document guild registration as the recommended development workflow.

**Exit criteria:** the local registry can be synchronized with Discord without a second handwritten command schema.

## Phase 7 — Subcommands and groups

Model Discord's native chat-input hierarchy directly.

- [ ] Add `#[group]`.
- [ ] Support `/group subcommand`.
- [ ] Support Discord-compatible subcommand groups.
- [ ] Resolve full command paths during dispatch.
- [ ] Expose the resolved command path from `Context<D>`.
- [ ] Enforce Discord hierarchy limits at compile time where possible.
- [ ] Detect duplicate paths deterministically.

**Exit criteria:** grouped command modules generate valid Discord subcommand trees and dispatch to the correct handler.

## Phase 8 — Choices and typed choice enums

Add static slash-command choices without falling back to untyped strings.

- [ ] Add inline choice metadata for supported scalar option types.
- [ ] Add `#[derive(CommandChoice)]`.
- [ ] Add `#[choice(name = "...")]` variant metadata.
- [ ] Generate Discord choice schemas from enums.
- [ ] Convert resolved choice values back into typed enum variants.
- [ ] Validate duplicate and invalid choice values at compile time where possible.

**Exit criteria:** handlers can receive typed enum choices while Discord receives matching static choice metadata.

## Phase 9 — Autocomplete

Support Discord application-command autocomplete as a first-class interaction path.

- [ ] Add `#[autocomplete]` handlers.
- [ ] Add `AutocompleteContext<D>` only if autocomplete-specific semantics justify a distinct type.
- [ ] Route autocomplete interactions separately from command execution.
- [ ] Expose the focused option and current partial value.
- [ ] Allow command options to reference autocomplete handlers.
- [ ] Validate autocomplete compatibility with the option type.
- [ ] Convert framework choices into Discord autocomplete responses.
- [ ] Enforce Discord result limits.

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
