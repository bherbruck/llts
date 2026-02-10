# Error Handling

## try/catch/throw as Sugar over Result

`throw`, `try`, and `catch` are familiar TS syntax that compiles to `Result<T, E>` branching — no LLVM exception machinery, no stack unwinding.

### What the developer writes:

```typescript
function parse(input: string): number {
  if (input === "") throw new Error("empty input");
  return parseFloat(input);
}

try {
  const n = parse(input);
  console.log(n);
} catch (e) {
  console.log("failed: " + e.message);
}
```

### What the compiler emits (conceptually):

```typescript
// throw makes the return type implicitly Result<number, Error>
function parse(input: string): Result<number, Error> {
  if (input === "") return Err(new Error("empty input"));
  return Ok(parseFloat(input));
}

const _result = parse(input);
if (_result.isOk()) {
  const n = _result.value;
  console.log(n);
} else {
  const e = _result.error;
  console.log("failed: " + e.message);
}
```

## Rules

- Functions containing `throw` implicitly return `Result<T, E>`. The compiler detects this from the function body.
- Calling a throwing function **without** `try`/`catch` is a compile error — forces explicit error handling.
- Nested `try`/`catch` — each level is a `Result` check.
- Rethrowing (`throw e` in a catch block) — returns the error up as `Err(e)`.
- `finally` — compiles to code emitted in both the Ok and Err branches (like a defer).
- Developers who prefer explicit `Result<T, E>` can use it directly — both styles work, same underlying representation.

## Why Not LLVM Exceptions

LLVM's `invoke`/`landingpad` exception machinery is complex to implement, has overhead in the non-throwing path (binary size, optimization barriers), and requires stack unwinding infrastructure. Result-based error handling is just return values and branches — simple, predictable, zero overhead when there's no error.
