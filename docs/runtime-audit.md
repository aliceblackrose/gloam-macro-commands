# Runtime audit and compatibility notes

## Runtime
- Managed dispatch isolates malformed interaction payloads and stale command paths. Gateway transport and registration errors remain fatal.
- Handler failures and join errors no longer terminate the managed loop. Enable the optional tracing feature and configure a subscriber to observe redacted failure events.
- Capacity rejection attempts an ephemeral busy response or empty autocomplete result. A separate 16-permit semaphore and two-second timeout bound this work; excess requests are dropped rather than queued.
- Contexts share parsed command data and traverse the validated zero-to-two-level option path rather than cloning leaf option values.

## Response state
An interrupted acknowledgement, edit, or deletion may have succeeded remotely. The framework marks state-changing requests uncertain before awaiting them and rejects further mutations after ambiguous errors. Definitive local validation or Discord invalid-form rejection restores the previous state. Explicitly call `Context::reconcile_response()` to recover when the original response can be retrieved. A failed lookup cannot safely distinguish an expired interaction, an unacknowledged interaction or a deleted original; the state intentionally remains uncertain. Explicit followup after deferral completes the original message and enforces the initial visibility.

## Registration and dependencies
Manual descriptors are checked at insertion for valid node and option names, descriptions, child/option limits, duplicate options, required-option ordering, supported kinds, numeric/length bounds and static choice metadata. The existing autocomplete checks remain in place.

Gloamwire is pinned to the audited upstream main commit `86633878229c4cbfaedfe2c5c524186f7bd683f9` rather than a moving branch. Update that revision deliberately and rerun CI. This pins Gloamwire's source but is not a complete lock of transitive registry dependencies.

## Validation
Regression tests cover cooldown expiration, oversized concurrency, macro adapter hygiene, descriptor validation and uncertain response state. Managed-mode overload and failure paths should also be exercised against a mocked or live Discord transport; live delivery and HTTP ambiguity cannot be guaranteed by local state-machine tests.
