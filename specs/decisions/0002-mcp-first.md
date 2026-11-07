# ADR-0002: MCP first

- Status: accepted
- Date: 2026-11-07

## Context

The main user of the tool is an AI agent that needs real runtime values instead of guessing them from the code. A person using the desktop app is the second user.

## Decision

Every capability is exposed through MCP. A feature is done when an MCP client can use it. The desktop app may present a capability differently, but it never has one that MCP lacks.

MCP tool names, descriptions and result shapes are part of the product contract. They are written in the milestone's `design.md` and reviewed by the [MCP contract reviewer](../reviewers/mcp-contract.md) before implementation.

## Alternatives rejected

- GUI first, with MCP added later. MCP would end up with a partial view of the features, which is the situation this project exists to avoid.
- A REST API as the primary contract, with MCP as a thin wrapper. Agents would get tools shaped by HTTP resources instead of by debugging tasks.

## Consequences

- The first implementation milestone delivers MCP tools before the desktop app exists.
- Tool results need explicit size limits, depth limits and pagination, because large payloads exhaust an agent's context.
- Long operations (waiting for a pause, waiting for logpoint hits) need tools that block with a timeout. Spike S3 tests how MCP clients handle that.
