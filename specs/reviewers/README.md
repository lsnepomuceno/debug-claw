# Reviewers

Prompts for reviews that run in a fresh context, with no conversation history. The reviewer gets only the document and the files it links to, so it stumbles where the document relies on something that was never written down. See [ADR-0007](../decisions/0007-spec-process.md).

| Prompt | Reviews | When |
|---|---|---|
| [implementation-readiness.md](implementation-readiness.md) | A milestone's `design.md` and `tasks.md` | Before the milestone moves to implementation |
| [mcp-contract.md](mcp-contract.md) | The MCP tool contract in a `design.md` | When a milestone adds or changes MCP tools |

## Running a review

Start one new agent per review. Give it the prompt file, the target document and the files the target links to, and nothing from the conversation that produced the document. The agent returns findings. The author decides which ones to act on.

## License

Everything in this folder is licensed under the Apache License 2.0 ([LICENSE](LICENSE)), whatever license the rest of the repository uses.

`implementation-readiness.md` is a modified version of `agents/implementation_readiness.md` from [google/elephant-goldfish-model](https://github.com/google/elephant-goldfish-model), commit `e9e1f06`. The file header lists the changes. `mcp-contract.md` was written for this project and uses the same structure.
