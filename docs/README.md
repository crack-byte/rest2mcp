# REST2MCP

REST2MCP is a Rust-based OpenAPI-to-MCP gateway that translates OpenAPI 3.x documents into MCP tool schemas and routes tool calls to backend REST endpoints.

## Goals

- Keep REST API contracts and MCP tool definitions synchronized from a single source of truth
- Support MCP JSON-RPC over Streamable HTTP and local stdio execution
- Enforce tool visibility and execution checks based on caller scope
- Require human confirmation for destructive operations
- Preserve auditability for all state-changing actions

## Project layout

- src/main.rs: Application entrypoint and transport selection
- src/config.rs: Environment and runtime configuration
- src/mcp.rs: OpenAPI parsing, tool generation, and routing metadata
- src/security.rs: Caller context, RBAC checks, confirmation tokens, and audit hooks
- src/openapi.rs: Sample OpenAPI input used for local/dev validation
- docs/: Project documentation and requirements

## Quick start

1. Install Rust 1.70+.
2. In the project root, run:

   cargo run

3. For the HTTP transport, use:

   curl http://127.0.0.1:3000/health

4. Post an MCP tool list request to:

   http://127.0.0.1:3000/mcp

Backend calls use the first URL in an OpenAPI 3 document's `servers` list, including its base path. Swagger 2.0 documents use `schemes`, `host`, and `basePath` (HTTPS is assumed if `schemes` is omitted). If neither format specifies a backend URL, the gateway falls back to `REST2MCP_API_BASE_URL` (default: `http://127.0.0.1:8080`). OpenAPI server URL variables use their declared defaults.

Operation tools retain `operationId` names and map path, query, header, cookie, and JSON body arguments. Required top-level arguments are checked before dispatch, local schema references are expanded, and duplicate generated tool names are rejected. OpenAPI serialization styles and remote `$ref` resolution are not fully supported.

## Environment variables

- REST2MCP_BIND: bind address, default 0.0.0.0:3000
- REST2MCP_TRANSPORT: stdio or streamable-http
- REST2MCP_LOG_TO_STDERR: true/false
- REST2MCP_ENABLE_UI: true/false, enables the lightweight web dashboard at /ui
- REST2MCP_API_BASE_URL: fallback API base URL for OpenAPI documents without a `servers` entry
- REST2MCP_DB_PATH: SQLite database file path, default `rest2mcp.sqlite3` in the working directory
- REST2MCP_AUTH_TOKEN: optional gateway bearer token; required when binding to a non-loopback address
- REST2MCP_AUTH_SCOPES: comma-separated scopes granted to that token; defaults to `read:resources`
- REST2MCP_REQUESTS_PER_MINUTE: process-wide MCP request limit, default `120`
- REST2MCP_BACKEND_BEARER_TOKEN: optional bearer credential injected into backend requests; tool arguments cannot override it
- REST2MCP_DUAL_PERSONA: true/false
- REST2MCP_ALLOW_HUMAN_SSO: true/false
- REST2MCP_ALLOW_SERVICE_ACCOUNTS: true/false

Saved OpenAPI specs, the active schema selection, and recent request logs are stored in SQLite. The dashboard lets you restore or delete saved specs and clear request logs. The database retains up to 10 saved specs and 1,000 request log entries.

HTTP defaults to loopback. For remote binding, configure a strong `REST2MCP_AUTH_TOKEN`; remote dashboard mutations additionally require the `admin:write` scope. Set scopes explicitly, for example `read:resources,write:resources,admin:write`, only when needed. The request limit is global to this process, not per identity. The current bearer-token setup is a basic deployment guard, not OAuth/SSO or a replacement for TLS and a production identity provider.

## Running tests

cargo test

## Current status

This repository is a production-oriented MVP baseline. It covers the major design points from the requirements document: OpenAPI-derived tool definitions, JSON-RPC routing, read/write scopes, confirmation gating for destructive actions, and request sanitization.

The next phases would include distributed token storage, real auth providers, OpenTelemetry, and backend HTTP execution against live APIs.
