# ADR-0007: Spec process and fresh-context reviews

- Status: accepted
- Date: 2026-11-07

## Context

The project is built by one person working with AI agents. Two published approaches were considered:

- SDD 2.0 (Adaptive Spec-Driven Development, Laura Soddu, 2025) scales the process with risk through four tiers chosen by a weighted score.
- The Elephant-Goldfish Model ([google/elephant-goldfish-model](https://github.com/google/elephant-goldfish-model)) has one agent with context draft a design and reviewers without the conversation history review it.

The author already uses a phase-based spec layout in other projects: one folder per milestone with `requirements.md` in EARS, `design.md` and `tasks.md`, one active milestone at a time, ADRs that record rejected alternatives, and a list of open questions.

## Decision

Use the existing phase-based layout, described in [specs/README.md](../README.md) and [CONVENTIONS.md](../CONVENTIONS.md).

From SDD 2.0, take only the idea of sizing the spec to the milestone: a small milestone can skip `design.md`.

From the Elephant-Goldfish Model, take the fresh-context review, at two points only: implementation readiness of each `design.md`, and the MCP tool contract. The reviewer prompts live in [`reviewers/`](../reviewers/README.md). The EGM plugin itself is not installed.

## Alternatives rejected

- SDD 2.0 tiers and scoring. The project has one stakeholder and no regulation, and the weighted score adds precision that the inputs do not have.
- The full EGM plugin. Its drafting skills write their own document layout, which would compete with `specs/` as a second source of truth. Its five-stage review starts many subagents for every document, and it was at version 0.1.1 when evaluated.

## Consequences

- Each review is one subagent that receives only the document and the files it links to.
- Reviewers cannot catch wrong assumptions about how Xdebug, PHP or MCP clients behave. Those are tested by spikes.
