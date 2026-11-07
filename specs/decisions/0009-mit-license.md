# ADR-0009: MIT license

- Status: accepted
- Date: 2026-11-07

## Context

The repository is public. Without a license, nobody can legally reuse the code.

## Decision

The repository is licensed under MIT ([LICENSE](../../LICENSE)). Crates declare `license = "MIT"` in their manifests.

`specs/reviewers/` stays under Apache-2.0, because `implementation-readiness.md` is derived from Apache-2.0 material ([reviewers README](../reviewers/README.md#license)).

## Alternatives rejected

- Apache-2.0, or MIT and Apache-2.0 together, which is common for Rust crates. MIT was the author's choice.

## Consequences

- Code copied from other projects must have a license compatible with MIT, and keeps its own notice.
