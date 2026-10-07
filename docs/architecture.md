# Architecture

## Overview

REST2MCP sits between an MCP client and one or more backend REST APIs. It ingests an OpenAPI document, generates MCP tool metadata, and routes requests to the correct backend endpoint while enforcing guardrails.

## Request flow

1. The active OpenAPI/Swagger document is loaded from the dashboard or restored from SQLite.
2. A tool registry is built from supported operations; the document's backend server URL and base path are selected.
3. MCP clients send JSON-RPC over the HTTP endpoint or line-delimited JSON over stdio.
4. The gateway resolves its configured caller context, applies scopes, and enforces the HTTP request limit.
5. It checks required arguments and risk. Destructive calls create a pending, short-lived approval instead of immediately calling the backend.
6. After approval, the gateway maps path/query/header/cookie/body arguments to an HTTP request.
7. The backend response or a JSON-RPC error is returned; operational metadata is written to the SQLite request log.

## Components

### GatewayService

The main runtime service owns the registry and security guard. It handles the tool listing and tool execution paths.

### ToolRegistry

This registry derives canonical tool names, method/path mappings, operation metadata, and risk classification from OpenAPI operations.

### SecurityGuard

This module enforces authorization, checks tool risk categories, issues approval tokens, and records audit-style events for state-changing actions.

### Config

Runtime configuration is loaded from environment variables so the same codebase can run in local, staging, and production environments.

### SQLite store

The SQLite store persists up to 10 named OpenAPI documents and their resolved backend URLs, the active schema name, and up to 1,000 operational request-log entries. Pending human approvals are intentionally in-memory and expire on restart.

## Transport model

### HTTP transport

- `GET /health` returns a basic readiness signal.
- `POST /mcp` accepts the gateway's JSON-RPC request shape and returns JSON-RPC result/error envelopes.
- `/ui` serves the dashboard; `/ui/status` and `/ui/logs` provide dashboard data.
- Dashboard mutations include spec load/restore/delete and log clearing; with gateway bearer auth enabled, mutations require `admin:write`.
- HTTP binds to loopback by default. Non-loopback binds require a configured static bearer token.

### stdio transport

- Reads JSON messages from stdin
- Writes JSON-RPC responses to stdout
- Keeps diagnostic logs on stderr to avoid corrupting the stream

## Security boundaries

- Tool visibility is filtered at schema advertisement time.
- Tool execution re-checks access before routing.
- Destructive tools require approval before execution proceeds.
- Request text is sanitized to reduce prompt-injection payloads.
- Tool calls use a configured backend bearer credential rather than exposing it as a tool argument.

## Extension points

The current structure is designed to be extended with:

- real OpenAPI file loading and hot reload
- external auth providers
- backend HTTP execution and timeouts
- audit/event storage to append-only logs
- distributed confirmation token storage

## Current scope limits

This is a prototype, not a complete MCP Streamable HTTP implementation or production identity gateway. Authentication is a single static bearer token with configured scopes; it is not OAuth/SSO or per-user authorization. Rate limiting is process-wide. OpenAPI mapping covers common parameter locations and local references, but not all serialization styles, remote references, or every OpenAPI feature. Review [security considerations](security.md) before deploying beyond a trusted local environment.
