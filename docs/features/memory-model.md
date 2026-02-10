# Memory Model

The developer writes normal TypeScript. The compiler manages memory automatically using a three-tier strategy.

## Tier 1: Stack Allocation (Default)

Primitives (`number`, `i32`, `boolean`, etc.) and small structs live on the stack. Passed by value, copied on assignment. This covers the majority of local variables and is essentially free.

## Tier 2: Escape Analysis

When a value must outlive its stack frame (returned, stored in a longer-lived struct), the compiler promotes it to the heap. Because we have full type information, monomorphized generics, and whole-program compilation, our escape analysis is more precise than Go's (no interface opacity or cross-package conservatism).

## Tier 3: Lobster-style Compile-Time Reference Counting

Heap-allocated objects use automatic reference counting. The compiler runs an ownership analysis pass (inspired by Lobster) that designates a single owner per allocation and treats all other uses as borrows. This eliminates ~95% of runtime refcount operations at compile time. The remaining ~5% get lightweight runtime retain/release.

| What happens | When | Developer sees |
|---|---|---|
| Stack allocation | Value doesn't escape the function | Nothing |
| Move (transfer ownership) | Value returned or stored, not used again | Nothing |
| Borrow (pointer, no RC) | Value passed to function that only reads it | Nothing |
| RC increment | Value shared across multiple owners (~5% of cases) | Nothing |
| RC decrement + free | Last owner goes out of scope | Nothing |
| Copy-on-write | Shared array/string mutated | Nothing |

## Cycles

Rejected at the type level. If the compiler detects a potentially cyclic type definition (type A contains type B contains type A), the developer must break the cycle with `Weak<T>`. This eliminates the entire class of ARC memory leak bugs. Tree-shaped data (the overwhelmingly common case) works without any annotation.

## Ownership & Borrowing

Ownership is fully inferred from usage. The compiler analyzes each function body to determine whether parameters are borrowed, mutably borrowed, or owned:

```typescript
// Compiler infers: a and b are immutable borrows (no RC, pass as pointer)
function distance(a: Point, b: Point): f64 {
  return Math.sqrt((a.x - b.x) ** 2 + (a.y - b.y) ** 2);
}

// Compiler infers: arr is mutably borrowed (push modifies it)
function appendZero(arr: number[]): void {
  arr.push(0);
}

// Compiler infers: p is moved/RC'd (stored in a longer-lived collection)
function storePoint(p: Point, list: Point[]): void {
  list.push(p);
}
```

For explicit contracts at API boundaries, use `Readonly<T>` — an existing TypeScript utility type that maps to an immutable borrow:

```typescript
// Explicit: "I promise not to mutate your data" — compiler enforces this
function hash(data: Readonly<Buffer>): u32 {
  // data.write(...) would be a COMPILE ERROR
  // Compiler: guaranteed borrow, no RC bump
}
```

No new ownership syntax. `Readonly<T>` is valid TypeScript, IDEs already support it, developers already know it.

## Implementation Phases

Each phase produces a working compiler:

1. Stack for primitives + naive RC everywhere on heap (correct first)
2. Escape analysis to reduce unnecessary heap allocations
3. Lobster-style ownership analysis to elide ~95% of retain/release
4. Type-level cycle prevention
