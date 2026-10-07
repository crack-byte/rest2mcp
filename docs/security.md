# Security and Governance

## Core principles

- Never expose tools to callers that are outside their authorized scope
- Classify tools by risk before executing them
- Log state-changing actions with enough context for audit review
- Require human confirmation before destructive operations execute
- Prevent prompt injection from being spread across tool descriptions and response text

## Risk levels

The current implementation assigns risk in the tool registry based on HTTP method:

- GET: Low
- POST: Medium
- PUT/PATCH: High
- DELETE: Critical

This aligns with the project requirements for Green/Yellow/Red mapping and supports future extension via OpenAPI x-* metadata.

## Authorization model

HTTP deployments bind to loopback by default. Binding to a non-loopback address requires `REST2MCP_AUTH_TOKEN`; `/mcp` requires that bearer token and receives only the scopes listed in `REST2MCP_AUTH_SCOPES` (read-only by default). Dashboard changes such as loading/deleting specs and clearing logs require `admin:write` when bearer auth is enabled. MCP requests are limited by `REST2MCP_REQUESTS_PER_MINUTE` (process-wide; default 120). Configure `REST2MCP_BACKEND_BEARER_TOKEN` to inject a backend credential; it overrides any caller-supplied Authorization header.

This static-token mechanism is a minimal deployment safeguard, not OAuth/SSO, per-user identity, a per-client rate limiter, or a substitute for TLS. Terminate TLS at a trusted proxy and use a production identity provider for multi-user deployments.

Tool-level access is checked against caller scopes. A caller must satisfy all required scopes for a tool before the tool is visible or executable.

Example:

- read-only tools require read:resources
- write tools require write:resources
- delete tools require admin:write

## Human approval

Destructive operations are identified by HTTP method/risk rather than tool name. A call returns a short-lived approval token without calling the backend; submit a JSON-RPC `tools/approve` request with that token after review. Approval is bound to the initiating caller, rechecks that caller's scopes, and is consumed once. The dashboard also offers an explicit confirmation button for pending calls.

The in-memory pending-approval store is lost on restart, which safely invalidates outstanding tokens. With the current shared static bearer token, approval is bound to that configured service identity, not an independently authenticated human identity.

The security layer can:

- identify destructive operations from their HTTP method/risk
- generate a single-use approval token
- retain the original call parameters until approval
- execute the approved operation once, after reauthorization
- log the request as a pending action
- reject replay or stale confirmation attempts

## Fragmentation defense

The gateway inspects request payload text and flags suspicious prompt-splitting patterns. This is a first-line defense against cross-channel prompt injection attacks and is intentionally simple but effective for a baseline implementation.

## Logging hygiene

In stdio mode, all logs go to stderr. The JSON-RPC stream remains on stdout only, preventing protocol corruption.

OpenAPI path/query/header/cookie/body parameter locations are preserved and mapped to their respective HTTP request locations. Local `$ref` schemas are expanded for tool argument and response schemas; unsupported OpenAPI serialization styles and remote references are not fully implemented.

## Future hardening

The following are planned for production deployment:

- real identity token verification
- per-user and per-service-account policy stores
- append-only audit log persistence
- Redis-backed approval tokens
- OpenTelemetry tracing and Prometheus metrics

## Current limitations

- The gateway token is a single static secret; there is no OAuth/OIDC verification, user directory, identity federation, or per-user scope mapping.
- HTTP uses the gateway's configured scopes, and stdio uses a built-in local prototype identity. Neither is equivalent to validating a real human identity.
- Rate limiting is in-memory and process-wide. It is not per caller, distributed, or persistent.
- Backend credential support currently injects a bearer token only; secret rotation and multiple backend auth schemes are not built in.
- Pending approvals are in-memory and lost on restart. A configured shared token means the approval subject identifies that token, not an individual reviewer.
- Dashboard and MCP HTTP endpoints should be placed behind TLS and trusted network controls if remotely exposed. Do not treat this baseline as a hardened internet-facing gateway.
- OpenAPI references and serialization are partial; remote references and all parameter styles are not supported.
