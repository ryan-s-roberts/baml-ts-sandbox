# baml-rt-quickjs

QuickJS-backed runtime host for BAML execution.

## Responsibilities

- `BamlRuntimeManager` orchestration for schema loading and function execution.
- `QuickJSBridge` integration to expose BAML functions to JavaScript.
- Context handling and JS value conversion utilities.

## Tool Session Contract

Host tools are invoked from BAML via `ToolSessionPlan` steps. The runtime
executes the session FSM in Rust and returns the final result to JS. JS tools
remain callable via `invokeTool`.
