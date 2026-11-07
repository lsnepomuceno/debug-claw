# Agent instructions

- Everything in this repository is in English: code, specs, docs, commit messages and MCP tool descriptions.
- Read [`specs/README.md`](specs/README.md) before starting work. Only the active milestone is built. Do not build ahead of it.
- Specs follow [`specs/CONVENTIONS.md`](specs/CONVENTIONS.md). A decision lives in one file. What is not decided goes to [`specs/OPEN.md`](specs/OPEN.md), never into an invented value.
- Memory and CPU use are requirements. Follow the rules and budgets in [`docs/performance.md`](docs/performance.md): nothing polls, every buffer has a cap, data loads on demand.
- Spike code lives in `spikes/` and is never imported by production code.
- Commit messages follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/): `type(scope): description`, imperative mood, lowercase description, no trailing period, body wrapped at 72 columns.
- Do not use em dashes or en dashes in prose. Use a colon, a comma, parentheses or two sentences.
