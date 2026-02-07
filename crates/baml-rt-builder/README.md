# baml-rt-builder

Agent build pipeline and packaging utilities.

## Responsibilities

- TypeScript/JavaScript linting and compilation via OXC.
- BAML type generation and schema handling for packaging.
- Agent packaging into distributable archives.

## Generated BAML Interfaces

The builder emits `generated_tools.baml`, which includes `ToolSessionPlan`
and `ToolSessionStep` definitions used by BAML to describe host tool session
execution steps.

## Binary

- `baml-agent-builder`: CLI entry point defined by this crate.
