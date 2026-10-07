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

Tool-level access is checked against caller scopes. A caller must satisfy all required scopes for a tool before the tool is visible or executable.

Example:

- read-only tools require read:resources
- write tools require write:resources
- delete tools require admin:write

## Human approval

Destructive operations are treated as requiring human confirmation. The security layer can:

- identify destructive tool names
- generate a single-use approval token
- log the request as a pending action
- reject replay or stale confirmation attempts

## Fragmentation defense

The gateway inspects request payload text and flags suspicious prompt-splitting patterns. This is a first-line defense against cross-channel prompt injection attacks and is intentionally simple but effective for a baseline implementation.

## Logging hygiene

In stdio mode, all logs go to stderr. The JSON-RPC stream remains on stdout only, preventing protocol corruption.

## Future hardening

The following are planned for production deployment:

- real identity token verification
- per-user and per-service-account policy stores
- append-only audit log persistence
- Redis-backed approval tokens
- OpenTelemetry tracing and Prometheus metrics
