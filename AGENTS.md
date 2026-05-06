# kubeview

## Project Scope

kubeview is a read-only Kubernetes MCP server. It exposes MCP tools for inspecting Kubernetes
clusters, not for changing them.

- Do not add tools or code paths that create, update, patch, delete, scale, restart, port-forward,
  exec into, attach to, or otherwise mutate Kubernetes resources.
- Do not shell out to `kubectl` for core behavior. Prefer the `kube` and `k8s-openapi` Rust APIs so
  requests stay typed, testable, and constrained by the project's read-only model.
- Kubernetes RBAC still applies; preserve clear error propagation when the configured identity lacks
  access.
- If namespace scoping is active, keep rejecting all-namespaces reads and cluster-scoped resource
  reads unless the security model is deliberately changed and documented.
- Treat pod logs as read-only inspection. Do not add previous-container, follow/streaming, or
  broad log collection behavior without considering access, resource usage, and tests.

## Rust Conventions

- Use idiomatic Rust with explicit error handling. Return `Result` for expected failures and avoid
  `unwrap()` in production code.
- Use `thiserror` for library/domain error types and `anyhow` only where application-level context
  is appropriate.
- Add context to errors when crossing external boundaries such as Kubernetes API calls, kubeconfig
  loading, server startup, and MCP request handling.
- Prefer typed structs/enums/newtypes over stringly typed APIs when representing resources, scopes,
  selectors, or tool inputs.
- Avoid bool or ambiguous `Option` parameters that make call sites hard to read. Prefer small enums,
  named helper methods, or clear input structs.
- Inline `format!` arguments when possible, e.g. `format!("pod {name}")`.
- Prefer exhaustive `match` statements. Avoid wildcard arms when new enum variants should force
  reconsideration.
- Keep public APIs small. Use `pub(crate)` unless an item is part of the crate's intended public
  surface.
- Do not introduce unsafe code.

## Module Organization

- Keep `main.rs` thin and put reusable behavior in library modules.
- Avoid growing large modules further. In this repository, treat these files as already large:
  - `src/kubernetes.rs`
  - `src/tools.rs`
- Prefer adding focused modules for new behavior instead of adding unrelated code to large files.
- Target modules under roughly 500 lines excluding tests. If a file exceeds roughly 800 lines, add
  new functionality in a new module unless there is a strong reason not to.
- Keep invariants close to the code that owns them. When extracting code, move related tests and
  module docs with the implementation.

## MCP Tool Design

- Tool names and schemas should stay stable unless the change is intentional and documented.
- Tool input structs should validate user-facing constraints at the boundary.
- Return structured, predictable JSON values. Avoid ad hoc string parsing or display-only responses
  when the caller may need to inspect fields.
- For new list-style tools, prefer explicit filters and bounded output. Avoid fetching unbounded
  cluster-wide data by default.
- Keep all tool behavior read-only and deterministic for the same Kubernetes API state.

## Tests

- Add or update tests when behavior changes, especially for:
  - namespace-scoped access restrictions
  - cluster-scoped access rejection
  - MCP tool input validation
  - Kubernetes API error handling
  - JSON response shape
- Prefer asserting whole structs or JSON values over piecemeal field assertions when practical.
- Use `#[tokio::test]` for async tests.
- Avoid mutating process-global environment in tests. Prefer explicit configuration objects or
  dependency injection.
- Tests should not require a live Kubernetes cluster unless they are clearly marked or documented as
  integration/manual tests.

## Integration Test Organization

- Keep live-cluster and kind-based integration tests isolated from unit tests and production modules.
- Live-cluster tests must be opt-in by default, for example through an explicit environment variable
  in CI.
- Place integration fixtures, cluster helpers, protocol clients, and assertions in focused test-only
  modules instead of growing one large test file.
- Organize assertions by behavior or scenario, not by a single broad end-to-end function.
- Avoid repeated ad hoc JSON path access in test bodies. Prefer small helper methods or test-only
  response structs for frequently asserted response shapes.
- Use cleanup guards for temporary Kubernetes resources, and name helpers according to their real
  responsibility.

## Local Verification

After Rust code changes, run the relevant checks from the repository root:

```bash
cargo +nightly fmt -- --check
cargo check --all
cargo clippy --all-targets --all-features --tests --benches -- -D warnings
cargo test
```

If only documentation changes were made, Rust checks are usually unnecessary.

## Dependency Changes

- Keep `Cargo.toml` dependency lists sorted and place new crates in the correct dependency section.
- After changing dependencies, update `Cargo.lock` in the same change.
- Prefer avoiding new dependencies unless they materially reduce complexity or improve correctness.

## Documentation

- Update `README.md` when CLI flags, MCP endpoint behavior, tool names, tool schemas, or the security
  model changes.
- Keep documentation in English.
- When explaining architecture or flows in longer documentation, use Mermaid diagrams where they
  clarify relationships or request flow.
