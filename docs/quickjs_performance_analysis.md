# QuickJS Synchronous eval() Analysis

## Understanding the "Limitation"

### What `eval()` Being Synchronous Actually Means

QuickJS's `eval()` is **synchronous only for the initial call** - it parses and executes JavaScript code immediately and returns a value. However, this is **NOT a limitation** for our architecture because:

1. **We're using Promises, not blocking on eval()**
   - When we call `JsValueFacade::new_promise()`, we create a promise that QuickJS's internal promise system handles asynchronously
   - The `eval()` call itself returns immediately with the promise object
   - The actual async work (BAML function execution) happens in Rust futures, not in QuickJS

2. **JavaScript's event loop handles awaiting**
   - Once JavaScript code gets the promise, it can await it normally
   - QuickJS's promise system runs the Rust futures asynchronously
   - No blocking occurs

### Performance Characteristics

#### eval() Overhead
- **Parse time**: Very fast (~microseconds for typical code)
- **Execution**: Immediate return (doesn't wait for async work)
- **Memory**: Minimal (~few KB per runtime)

#### Actual Async Work
- BAML function execution happens in Rust (Tokio async runtime)
- QuickJS is just a thin bridge layer
- Multiple BAML calls can run concurrently regardless of eval() being synchronous

## Architecture: Runtime Per Context

### Current Architecture

```
┌─────────────────┐
│ QuickJS Bridge  │  (Single instance, shared)
│  (One Runtime)  │
└────────┬────────┘
         │
         ├─> BAML Function Call ──> Rust (Async) ──> LLM API
         │
         └─> Another Call ─────────> Rust (Async) ──> LLM API
```

### Proposed: Runtime Per Context

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ QuickJS Runtime │     │ QuickJS Runtime │     │ QuickJS Runtime │
│  (Context A)    │     │  (Context B)    │     │  (Context C)    │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                       │
         └───────┬───────────────┴───────────────┬───────┘
                 │                               │
         ┌───────▼───────────────────────────────▼───────┐
         │      Shared BAML Runtime Manager              │
         │           (Rust Async Runtime)                │
         └───────────────────────────────────────────────┘
```

### Benefits of Runtime-Per-Context

1. **Isolation**
   - Each context has its own JavaScript global scope
   - No variable/state pollution between contexts
   - Safer for multi-tenant scenarios

2. **Concurrency**
   - Multiple contexts can execute JavaScript simultaneously
   - No contention on QuickJS runtime (though eval() is fast anyway)
   - Better parallelization

3. **Lightweight**
   - QuickJS runtimes are tiny (~few KB each)
   - Can spawn hundreds/thousands easily
   - Minimal memory overhead

4. **Cleaner Architecture**
   - Each user/request gets isolated context
   - Easier to reason about state
   - Better for serverless/multi-tenant

### Performance Comparison

#### Single Runtime (Current)
- **Memory**: ~50-100 KB (one QuickJS instance)
- **Throughput**: Limited by BAML execution (not QuickJS)
- **Latency**: eval() overhead ~1-10 microseconds
- **Concurrency**: All contexts share one runtime (but eval() is fast)

#### Runtime Per Context
- **Memory**: ~50-100 KB × N contexts
- **Throughput**: Same (bottleneck is BAML/LLM, not QuickJS)
- **Latency**: Same (~1-10 microseconds per eval())
- **Concurrency**: Better isolation, true parallel JavaScript execution

### Bottleneck Analysis

The **actual bottleneck** is NOT QuickJS eval():

1. **BAML Function Execution**: 100-5000ms (LLM API calls)
2. **Network I/O**: Variable (depends on LLM provider)
3. **JSON Serialization**: ~0.1-1ms (we've optimized this)
4. **QuickJS eval()**: ~0.001-0.01ms (negligible)

**Conclusion**: QuickJS eval() being synchronous has **zero performance impact** on throughput because:
- It's not the bottleneck (BAML/LLM is)
- It returns immediately (doesn't block)
- Async work happens in Rust anyway

## Recommendations

### Use Runtime Per Context When:
- ✅ Multi-tenant scenarios (need isolation)
- ✅ Need separate global scopes per user/request
- ✅ Running in serverless environments
- ✅ Want true parallel JavaScript execution
- ✅ Memory is not a concern (runtimes are tiny)

### Single Shared Runtime When:
- ✅ Single-user application
- ✅ Memory-constrained environments
- ✅ All contexts share the same global scope
- ✅ Simpler architecture is preferred

### Implementation Strategy

```rust
// Per-context runtime (recommended for production)
pub struct BamlContext {
    quickjs: QuickJSBridge,
    // ... other context-specific data
}

impl BamlContext {
    pub fn new(baml_manager: Arc<Mutex<BamlRuntimeManager>>) -> Result<Self> {
        Ok(Self {
            quickjs: QuickJSBridge::new(baml_manager)?,
            // ...
        })
    }
}

// Usage: One context per user/request
let context = BamlContext::new(baml_manager.clone())?;
context.quickjs.register_baml_functions().await?;
let result = context.quickjs.evaluate(code).await?;
```

## Conclusion

**QuickJS's synchronous eval() is NOT a limitation** - it's actually perfect for our use case:
- Returns immediately (non-blocking)
- Async work happens in Rust (where it should be)
- JavaScript can await promises normally
- Performance impact is negligible (~0.001ms overhead)

**Runtime-per-context is recommended** for production for isolation and cleaner architecture, but the performance benefit is from isolation, not from avoiding the "limitation" (which isn't actually limiting).



