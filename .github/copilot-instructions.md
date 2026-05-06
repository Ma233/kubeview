# kubeview Copilot Instructions

kubeview is a read-only Kubernetes MCP server. Treat the read-only boundary as a security invariant.

When reviewing or generating code for this repository:

- Do not introduce MCP tools, handlers, helpers, tests, examples, or documentation that create,
  update, patch, delete, scale, restart, port-forward, exec into, attach to, or otherwise mutate
  Kubernetes resources.
- Do not shell out to `kubectl` for core behavior. Prefer typed, read-only use of the `kube` and
  `k8s-openapi` Rust APIs.
- Treat pod logs as read-only inspection only. Flag broad log collection, previous-container access,
  follow/streaming logs, or behavior that could materially increase access or resource usage.
- Preserve namespace-scoped restrictions. If namespace scoping is active, all-namespaces reads and
  cluster-scoped reads should remain rejected unless the security model is deliberately changed,
  documented, and tested.
- Preserve clear Kubernetes RBAC error propagation when the configured identity lacks access.
- Keep MCP tool names and schemas stable unless a change is intentional, documented, and tested.
- Prefer structured JSON responses over display-only strings when callers may inspect returned
  fields.
- Validate user-facing tool inputs at the MCP boundary.
- Add or update tests for namespace restrictions, cluster-scoped access rejection, input validation,
  Kubernetes API error handling, and JSON response shape when related behavior changes.

Pay special attention to changes involving:

- Rust `kube::Api` methods such as `create`, `update`, `patch`, `delete`, `delete_collection`,
  `replace`, `evict`, or subresource operations that can mutate cluster state.
- Any `Command`, shell, script, or documentation path that invokes mutating `kubectl` subcommands,
  including resource creation, deletion, patching, replacement, scaling, rollout mutation,
  interactive pod access, attachment, or port forwarding.
- New MCP tools or schemas that expose mutating verbs, ambiguous resource operations, unbounded
  cluster-wide reads, or broad log access.
- Abstractions that hide Kubernetes operations behind generic method names such as `run`, `apply`,
  `sync`, `reconcile`, `ensure`, or `execute`.

For Rust changes:

- Use idiomatic Rust with explicit error handling. Avoid `unwrap()` in production code.
- Use `thiserror` for library/domain error types and `anyhow` where application-level context is
  appropriate.
- Add context when crossing external boundaries such as Kubernetes API calls, kubeconfig loading,
  server startup, and MCP request handling.
- Prefer typed structs, enums, and newtypes over stringly typed APIs for resources, scopes,
  selectors, and tool inputs.
- Keep `src/main.rs` thin and avoid growing already-large modules such as `src/kubernetes.rs` and
  `src/tools.rs` with unrelated behavior.
- Do not introduce unsafe code.

If a proposed change appears to alter the read-only security model, call that out explicitly and ask
for design documentation and tests before accepting it.
