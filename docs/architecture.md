# Architecture

## Overview

REST2MCP sits between an MCP client and one or more backend REST APIs. It ingests an OpenAPI document, generates MCP tool metadata, and routes requests to the correct backend endpoint while enforcing guardrails.

## Request flow

1. OpenAPI spec is loaded into memory.
2. Tool registry is built from each operation and path.
3. MCP clients call tools via JSON-RPC.
4. The gateway validates caller identity and required scopes.
5. The gateway checks risk classification and HITL requirements.
6. The request is routed to the target HTTP endpoint.
7. Results are transformed back into MCP-compatible output.

## Components

### GatewayService

The main runtime service owns the registry and security guard. It handles the tool listing and tool execution paths.

### ToolRegistry

This registry derives canonical tool names, method/path mappings, operation metadata, and risk classification from OpenAPI operations.

### SecurityGuard

This module enforces authorization, checks tool risk categories, issues approval tokens, and records audit-style events for state-changing actions.

### Config

Runtime configuration is loaded from environment variables so the same codebase can run in local, staging, and production environments.

## Transport model

### HTTP transport

- `GET /health` returns a basic readiness signal.
- `POST /mcp` accepts MCP JSON-RPC payloads.
- This is the default mode for multi-client remote gateway usage.

### stdio transport

- Reads JSON messages from stdin
- Writes JSON-RPC responses to stdout
- Keeps diagnostic logs on stderr to avoid corrupting the stream

## Security boundaries

- Tool visibility is filtered at schema advertisement time.
- Tool execution re-checks access before routing.
- Destructive tools require approval before execution proceeds.
- Request text is sanitized to reduce prompt-injection payloads.

## Extension points

The current structure is designed to be extended with:

- real OpenAPI file loading and hot reload
- external auth providers
- backend HTTP execution and timeouts
- audit/event storage to append-only logs
- distributed confirmation token storage
