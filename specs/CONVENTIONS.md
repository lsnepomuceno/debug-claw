# Spec conventions

## Language

Everything in the repository is in English: code, specs, docs, commit messages and MCP tool descriptions. See [ADR-0006](decisions/0006-english-repository.md).

## Writing rules

A decision lives in one file only. If two files define the same thing, one of them is wrong. Link to it instead of repeating it.

Requirements describe observable behavior, not implementation. "When a session pauses on a breakpoint, the `wait_for_pause` tool returns the file and line" is a requirement. "The session registry is a `DashMap`" belongs in `design.md` or in a code comment.

Requirements do not name technologies, except the external protocols the tool must speak (DBGp, MCP, HTTP) and the external software it integrates with (Xdebug, PHP, the IDE).

Every requirement needs a test. If you cannot write the test that proves the requirement holds, the requirement is too vague to implement.

Every design block points to the requirement it serves (`Serves: R-03`). Design does not invent requirements.

Every task closes at least one acceptance criterion. A task that closes none should not exist.

What has not been decided goes to [OPEN.md](OPEN.md), with who decides and what it blocks. Never fill a gap with an invented value: a wrong number that looks decided is worse than an empty field.

## Requirement format

Requirements use EARS:

- Ubiquitous: `The <system> shall <response>.`
- Event driven: `When <trigger>, the <system> shall <response>.`
- State driven: `While <state>, the <system> shall <response>.`
- Unwanted behavior: `If <condition>, then the <system> shall <response>.`
- Optional feature: `Where <feature is enabled>, the <system> shall <response>.`

Each requirement has an ID (`R-01`) that is unique within the milestone, and its acceptance criteria are listed under it.

Spike milestones are the exception. A spike produces knowledge, not behavior, so its requirements are questions, each with exit criteria that say what evidence answers it.

## Decisions

Decisions are ADRs in [`decisions/`](decisions/), numbered `NNNN-short-name.md`. Each one has a status (`proposed`, `accepted`, `superseded by NNNN`), a date, the context, the decision, the alternatives that were rejected with the reason, and the consequences. A rejected alternative is written down so that nobody, human or agent, proposes it again without new information.

A `proposed` ADR records a direction that a spike or a milestone still has to confirm. Code may follow it, but the ADR is not final until it is `accepted`.

## Spike code

Spike code lives in [`/spikes`](../spikes/), one folder per spike. It is throwaway: production crates never depend on it, and it is not held to the production quality bar. What a spike learns goes into its report in `specs/00-spikes/reports/`, and from there into ADRs and OPEN.md.
