# kubeview

Read-only Kubernetes MCP server for inspecting clusters from MCP clients.

## Usage

Start the MCP server with the current kubeconfig context:

```bash
cargo run -- serve
```

The default endpoint is:

```text
http://127.0.0.1:3000/mcp
```

Select a kubeconfig, context, namespace, host, port, or path:

```bash
cargo run -- serve \
  --kubeconfig ~/.kube/config \
  --context minikube \
  --namespace default \
  --host 127.0.0.1 \
  --port 3000 \
  --path /mcp
```

If `--namespace` is set, kubeview restricts namespace-scoped reads to that namespace and rejects all-namespaces reads. Cluster-scoped resources are also rejected while a namespace scope is active.

## MCP Client Configuration

Use the streamable HTTP endpoint from your MCP client:

```json
{
  "mcpServers": {
    "kubeview": {
      "url": "http://127.0.0.1:3000/mcp"
    }
  }
}
```

## Tools

kubeview exposes read-only tools:

- `list_contexts`
- `current_context`
- `list_namespaces`
- `list_pods`
- `get_pod`
- `pod_logs`
- `list_resources`
- `get_resource`

## Docker

Run a released image with a read-only kubeconfig mount:

```bash
docker run --rm \
  -p 3000:3000 \
  -v "$HOME/.kube/config:/home/kubeview/.kube/config:ro" \
  ghcr.io/ma233/kubeview:latest \
  serve --host 0.0.0.0 --port 3000
```

## Development

Run checks locally:

```bash
cargo +nightly fmt -- --check
cargo check --all
cargo clippy --all-targets --all-features --tests --benches -- -D warnings
cargo test
```

Run pre-commit hooks:

```bash
pre-commit run --all-files
```

## Security Model

kubeview is intentionally read-only. It never creates, updates, patches, deletes, or executes resources in the cluster. Kubernetes RBAC still applies, so run it with a kubeconfig whose permissions match the access you want MCP clients to have.
