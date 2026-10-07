# Setup and operator flow

This guide describes how to build and run the gateway, load an API contract, make a test call, handle a destructive-operation approval, and manage persisted data. The shorter overview is in the [root README](../README.md).

## 1. Build and start locally

Install stable Rust, then run from the project root:

```sh
cargo run
```

The default HTTP listener is `127.0.0.1:3000`, and the dashboard is enabled. Verify the process:

```sh
curl -i http://127.0.0.1:3000/health
```

A healthy response returns HTTP 200 with body `ok`. Open [http://127.0.0.1:3000/](http://127.0.0.1:3000/) for the dashboard.

Optionally choose a database path and fallback backend before startup:

```sh
REST2MCP_DB_PATH="$HOME/.local/share/rest2mcp/gateway.sqlite3" \
REST2MCP_API_BASE_URL="http://127.0.0.1:8080" \
cargo run
```

The parent folder for the SQLite path is created when needed. The database keeps up to 10 saved specs, the active spec name, and up to 1,000 request logs. The default file is `rest2mcp.sqlite3` in the current directory.

## 2. Load an API contract

In the dashboard's **OpenAPI Spec Loader**:

1. Choose a JSON/YAML file or paste the contract into the text area.
2. Optionally set a name. If blank, a timestamp-based name is generated.
3. Choose **Validate & Load**.
4. Check the active schema, backend URL, generated tool list, and tool schemas.

The first OpenAPI 3 `servers[].url` is used, including its base path. Server URL variable placeholders are replaced with their `default` values. For Swagger 2.0, the URL is assembled from `schemes`, `host`, and `basePath`; HTTPS is assumed when `schemes` is absent. If these fields are absent, the process falls back to `REST2MCP_API_BASE_URL`, defaulting to `http://127.0.0.1:8080`.

A spec reload replaces the active tool registry and saves the raw source and resolved backend URL. On process restart, the last active saved spec is restored. This prototype uses the first server only; server selection and per-operation server overrides are not implemented.

## 3. Try a tool in the dashboard

1. In **Manual MCP Tester**, choose a generated tool.
2. **Generate dummy request** builds an editable request from the available schema examples, defaults, enums, and basic type hints.
3. Inspect and adjust path, query, header, cookie, and body fields as required by the API.
4. Choose **Send request**.
5. Inspect the MCP response and **Recent Request Logs**. Logs include request name, tool, backend URL, outcome, HTTP status, and duration; they intentionally omit request parameters and response bodies.

The backend must be reachable from the gateway process. If a call fails, first check that the active server URL is correct and that the backend is available. Path/query/header/cookie/body mapping supports common cases, but OpenAPI serialization styles, remote `$ref`, and every schema feature are not implemented. Only configured backend bearer tokens are injected automatically; custom auth schemes need additional integration.

Saved specs have **Load** and **Delete** controls. Load switches the active spec and its backend URL. To delete a spec, load another one first; the active spec cannot be deleted. **Clear logs** removes operational request history from the database.

## 4. Call the HTTP endpoint directly

List tools:

```sh
curl -sS http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'
```

Call the Petstore status search operation (after loading a compatible contract):

```sh
curl -sS http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"findpetsbystatus","status":"available"}}'
```

Tool arguments are currently flattened next to `name` in `params`; this differs from clients that nest them under `arguments`. The gateway uses JSON-RPC request IDs and returns JSON-RPC error envelopes for application errors. HTTP transport uses this JSON endpoint; full Streamable HTTP session semantics are not implemented.

## 5. Configure access control

HTTP defaults to loopback. To bind to a non-loopback address, a static gateway token is required:

```sh
REST2MCP_BIND=0.0.0.0:3000 \
REST2MCP_AUTH_TOKEN='replace-with-a-long-random-secret' \
REST2MCP_AUTH_SCOPES='read:resources,write:resources,admin:write' \
cargo run
```

Treat the token as a secret: do not put real values in source control or pass production secrets on a shared shell command line. Prefer a protected service manager/secret store. Terminate TLS at a trusted reverse proxy.

For authenticated HTTP requests, send:

```sh
-H 'Authorization: Bearer <gateway-token>'
```

`REST2MCP_AUTH_SCOPES` controls the one configured token's scopes. Read tools need `read:resources`; write tools need `write:resources`; destructive tools need `admin:write`. Loading/deleting specs and clearing logs also require `admin:write` when token auth is configured. The default scope is read-only. The request limit (`REST2MCP_REQUESTS_PER_MINUTE`, default `120`) is global to the process rather than per identity.

To attach a backend credential, configure `REST2MCP_BACKEND_BEARER_TOKEN`. It is sent as `Authorization: Bearer ...` to backend APIs and overrides a tool-call-supplied Authorization header. Do not use it when the backend expects a different auth scheme without extending the client.

## 6. Review a destructive operation

A DELETE or Critical-risk tool call does not immediately contact the backend. It returns an `approval_required` result with an expiring approval token. The anonymous local HTTP caller is read-only, so to exercise this flow locally, start the gateway with a temporary token and the required scopes:

```sh
REST2MCP_AUTH_TOKEN='local-development-secret' \
REST2MCP_AUTH_SCOPES='read:resources,write:resources,admin:write' \
cargo run
```

Enter that token in the dashboard's **Gateway bearer token** field and choose **Use token**. Inspect the displayed method, path, and tool name. If the operation is intended, use the dashboard's **Approve pending action** confirmation or call `tools/approve`:

```sh
curl -sS http://127.0.0.1:3000/mcp \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer <gateway-token>' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/approve","params":{"approval_token":"<approval-token>"}}'
```

Approval is bound to the caller identity, rechecks scopes, expires after five minutes, and is consumed once. Pending approvals are memory-only and become invalid when the gateway restarts. With the configured shared-token mode, caller identity represents that static service token, not a distinct human user; only use the flow in trusted local/development scenarios until real identity integration is added.

## 7. Run as stdio

For a client that launches a local process and speaks line-delimited JSON-RPC on stdio:

```sh
REST2MCP_TRANSPORT=stdio cargo run
```

Configure that command and required environment variables in the MCP client's server settings. Stdio mode reads one JSON-RPC message per input line and writes one response per line to stdout; logs go to stderr. The current stdio identity is a local prototype identity, not an externally authenticated principal.

## 8. Tests and cleanup

Run checks:

```sh
cargo test
cargo check
```

To reset locally saved data, stop the gateway and remove the SQLite database file selected by `REST2MCP_DB_PATH` (or the default `rest2mcp.sqlite3`). This deletes saved specs, active selection, and request history. Build output under `target/` is generated and does not need to be committed.

## Environment variable reference

| Variable | Description | Default |
| --- | --- | --- |
| `REST2MCP_BIND` | HTTP listen address | `127.0.0.1:3000` |
| `REST2MCP_TRANSPORT` | `streamable-http` or `stdio` | `streamable-http` |
| `REST2MCP_ENABLE_UI` | Enable dashboard | `true` |
| `REST2MCP_API_BASE_URL` | Fallback URL when spec has no server | `http://127.0.0.1:8080` |
| `REST2MCP_DB_PATH` | SQLite file | `rest2mcp.sqlite3` |
| `REST2MCP_AUTH_TOKEN` | Static gateway bearer token; mandatory for remote bind | unset |
| `REST2MCP_AUTH_SCOPES` | Scopes granted to gateway token | `read:resources` |
| `REST2MCP_REQUESTS_PER_MINUTE` | Process-wide MCP request limit | `120` |
| `REST2MCP_BACKEND_BEARER_TOKEN` | Backend bearer credential | unset |
| `REST2MCP_LOG_TO_STDERR` | Route tracing logs to stderr | `true` |
| `REST2MCP_DUAL_PERSONA` | Reserved auth configuration flag | `true` |
| `REST2MCP_ALLOW_HUMAN_SSO` | Reserved auth configuration flag | `true` |
| `REST2MCP_ALLOW_SERVICE_ACCOUNTS` | Reserved auth configuration flag | `true` |
