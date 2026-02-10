# Async/Await (v2)

Rejected in v1. Using `async`/`await` produces a compile error with a clear message.

## v2 Path: State Machine Transform (Rust-style)

`async fn` compiles to a state machine struct where each `await` is a yield point. Needs an executor but no OS-level runtime.

Each `async` function is transformed into a struct that implements a `poll()` method. The struct holds the function's local variables and a state discriminant indicating which `await` point it's suspended at.

```typescript
// What the developer writes:
async function fetchAndParse(url: string): Promise<Data> {
  const response = await fetch(url);
  const text = await response.text();
  return parse(text);
}

// Conceptually compiles to a state machine:
// State 0: call fetch(url), suspend
// State 1: receive response, call response.text(), suspend
// State 2: receive text, call parse(text), return result
```

## Open Questions for v2

- What executor model? (single-threaded event loop, thread pool, pluggable?)
- How does `Promise<T>` map to the state machine's `Future`-like type?
- I/O integration — need async versions of file/network operations
