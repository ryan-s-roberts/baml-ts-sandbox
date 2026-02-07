# Testing Handbook

Authoritative reference for how we test surfaces in this repo. The goal is to
exercise systems the way real users (and unreliable networks) do. Happy paths
are nice, adversarial slices keep us honest. This is a living document: update
it as code evolves and use the examples below as patterns to adapt, not fixed
recipes.

---

## Testing Philosophy

- **Prefer vertical slices over unit shards.** Exercise the public API of the
  system under test (BAML runtime, A2A handlers, provenance writer, CLI entrypoint)
  and let real dependencies run. The only code that may be "test only" is
  fixture scaffolding (e.g. test support helpers, testcontainers, fixtures).
- **Use the test-support crate for shared fixtures.** Common setup helpers live
  in `crates/test-support` and are reused across tests to keep setup consistent.
- **Tests are contracts, not documentation.** Assertions should verify behavior
  that matters: specific outputs, error conditions, state transitions, and
  invariants (e.g. provenance normalization, tool registration, protocol flow).
- **Adversaries welcome.** For every happy path add malformed inputs, retries,
  and timing edge cases. Production bugs arrive from edges, not averages.
- **Make async behavior explicit.** Prefer `#[tokio::test]` for async surfaces,
  and keep concurrency controlled and deterministic where possible.

---

## Test Layout and Fixtures

### Where Tests Live

- Crate-level tests: `crates/*/tests/*.rs`
- Shared fixtures: `tests/fixtures/` (agent fixtures under `tests/fixtures/agents/`)
- Test utilities: `crates/test-support`

### Fixture Helpers (test-support)

Use `crates/test-support` for setup and fixtures rather than duplicating logic:

- `setup_baml_runtime_default()` and `setup_baml_runtime_from_fixture()` for runtime setup
- `setup_bridge()` for QuickJS bridge setup
- `agent_fixture()` and `fixture_path()` for fixture files
- `require_api_key()` to gate tests that require `OPENROUTER_API_KEY`
- `ensure_baml_src_exists()` to skip tests when `baml_src` is missing

---

## Integration and E2E Tests

### Use the Real API Surface

- Call the same functions that production code uses. Do **not** reach into
  `#[cfg(test)]` helpers to bypass validation or internal state.
- Seed data through production APIs, not internals. If you need a helper, add it
  to `crates/test-support` and call production surfaces from there.
- Prefer `BamlRuntimeManager`, `QuickJSBridge`, `A2aRequestHandler`, and
  provenance writers as the main entry points.

### External Services

Some tests use real infrastructure:

- `crates/baml-rt-provenance/tests/falkordb_store_test.rs` starts FalkorDB via
  `testcontainers` and validates persisted graph state.
- These tests are slower; keep them scoped and ensure cleanup completes before
  the container is dropped.

---

## Snapshot Testing

We use `insta` for JSON snapshots in provenance tests:

- Snapshots live in `crates/baml-rt-provenance/tests/snapshots/`
- Example usage: `insta::assert_json_snapshot!` in
  `crates/baml-rt-provenance/tests/falkordb_store_test.rs`
- Update snapshots with `cargo insta review` after intentional changes

Keep snapshot inputs deterministic where possible (fixed IDs, stable ordering,
normalized data) to avoid noisy churn.

---

## Adversarial Testing

- For every new feature, brainstorm how it fails: duplicate requests, malformed
  inputs, replayed messages, stale graph state, unexpected tool payloads.
- When writing an “expected” success test, ask: _What happens if we swap this
  ID, if two clients race, if the runtime returns a partial result?_ Add those
  cases.
- Favor explicit assertions that explain _why_ the behavior is required.

---

## Invariants and Behavior Contracts

Invariants should be encoded as direct assertions in tests, especially in:

- Provenance normalization and relation derivation
  (`crates/baml-rt-provenance/tests/normalizer_test.rs`)
- Provenance persistence and graph shape
  (`crates/baml-rt-provenance/tests/store_test.rs`)
- Tool registration and execution correctness
  (`crates/baml-rt/tests/tool_calling_test.rs`)

### How to Discover New Invariants

- **Start from the contract.** Walk the public API surface and ask: _what must
  be true before and after every call?_
- **Trace data flow across boundaries.** Follow a request through layers
  (runtime → bridge → tool execution → provenance).
- **Interrogate failure modes.** Review TODOs and past failures; turn them into
  assertions.
- **Consider conservation and exclusivity.** Look for quantities or relationships
  that must never be violated (single tool registration, unique IDs, graph edge
  consistency).

### Capturing the Invariants

1. **Name them explicitly.** Document each invariant inside the test module.
2. **Encode with helpers.** Wrap complex checks in reusable assertion functions.
3. **Add negative examples.** Create failing fixtures where useful.
4. **Keep invariants living.** Update them when behavior evolves.

---

## Concurrency and Async Testing

Most async tests use `#[tokio::test]`. When adding concurrency:

- Keep the test deterministic by controlling inputs and expected outcomes.
- Prefer explicit synchronization (`join!`, `try_join!`, and task joins).
- Ensure shared resources are not leaked between tests (fixtures are scoped per test).

---

## Example Playbook

1. **Integration test a new runtime feature.**
   - Use `setup_baml_runtime_default()` or `setup_baml_runtime_from_fixture()`.
   - Drive the public runtime APIs; no internal shortcuts.
   - Add both happy path and adversarial inputs.

2. **Provenance change.**
   - Update normalization tests and store tests.
   - Add or adjust `insta` snapshots if the graph shape changes.

3. **New tool or bridge behavior.**
   - Register the tool via `BamlRuntimeManager`.
   - Verify execution and JS bridge registration via `QuickJSBridge`.

---

## Quick Checklist Before Shipping

- Does the test call only production APIs (fixtures aside)?
- Are failure cases covered, not just success?
- Are snapshots deterministic and reviewed after changes?
- Are async/concurrency behaviors explicitly asserted?
- Are shared fixtures and external services properly scoped and cleaned up?

Following these practices keeps the test suite reliable as the codebase evolves.
