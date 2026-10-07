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

## Environment variables

- REST2MCP_BIND: bind address, default 0.0.0.0:3000
- REST2MCP_TRANSPORT: stdio or streamable-http
- REST2MCP_LOG_TO_STDERR: true/false
- REST2MCP_ENABLE_UI: true/false, enables the lightweight web dashboard at /ui
- REST2MCP_DUAL_PERSONA: true/false
- REST2MCP_ALLOW_HUMAN_SSO: true/false
- REST2MCP_ALLOW_SERVICE_ACCOUNTS: true/false

## Running tests

cargo test

## Current status

This repository is a production-oriented MVP baseline. It covers the major design points from the requirements document: OpenAPI-derived tool definitions, JSON-RPC routing, read/write scopes, confirmation gating for destructive actions, and request sanitization.

The next phases would include distributed token storage, real auth providers, OpenTelemetry, and backend HTTP execution against live APIs.
