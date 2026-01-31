# baml-agent-runner

CLI for loading and executing packaged agents.

## Responsibilities
- Load and validate packaged agent archives.
- Initialize QuickJS runtime and register BAML functions.
- Handle A2A requests over stdio and invoke JS-exposed functions.
