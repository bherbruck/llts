# Type System

## Primitive & Built-in Types

| TypeScript | LLVM IR | Notes |
|---|---|---|
| `number` | `f64` | Default JS number semantics |
| `i8`, `i16`, `i32`, `i64` | `i8`, `i16`, `i32`, `i64` | Ambient declarations in prelude |
| `u8`, `u16`, `u32`, `u64` | `i8`, `i16`, `i32`, `i64` | Unsigned, same LLVM type, different ops |
| `f32`, `f64` | `float`, `double` | |
| `boolean` | `i1` | |
| `string` | `{ ptr, len }` | Fat pointer to UTF-8 data |
| `void` | LLVM `void` | Function returns nothing |
| `null`, `undefined` | Null variant of `Option<T>` | Collapsed into one concept |
| `never` | Unreachable | Function never returns |
| `bigint` | Rejected (v1) | No arbitrary precision without runtime |
| `symbol` | Rejected | Dynamic, requires runtime registry |
| `object` | Rejected | No known layout |

`null` and `undefined` are collapsed into a single "absence" concept. `T | null`, `T | undefined`, and `T | null | undefined` all compile to `Option<T>`.

## Compound Types

| TypeScript | LLVM IR | Notes |
|---|---|---|
| `interface` / `type` object shape | `%struct.Name` | Named struct type |
| `type` union (`A \| B`) | `{ i32, union(A, B) }` | Tagged union |
| `type` alias (`type X = Y`) | Resolves to `Y` | No new LLVM type |
| `enum` | `{ i32, payload }` | Tagged union |
| `T[]` | `{ ptr, len, cap }` | Vec-like, heap allocated |
| `[T; N]` / tuple | `[N x T]` | Stack allocated fixed array |
| Generics `T` | Monomorphized | Specialized at each call site |
| `Option<T>` | `{ i1, T }` | Null pointer opt for pointer types |
| `Result<T, E>` | `{ i32, union(T, E) }` | Tagged union |
| Function type `(A) => B` | `{ fn_ptr, env_ptr }` | Fat pointer (closure representation) |

## `type` vs `interface`

Both `type` and `interface` compile to the same LLVM struct when defining an object shape. No preference, no special rules — write whichever you'd normally write:

```typescript
// These compile to the exact same LLVM struct:
type Point = { x: f64; y: f64 };
interface Point { x: f64; y: f64 }
// → %struct.Point = type { double, double }
```

`type` can additionally express things `interface` cannot:

```typescript
type Num = i32;                    // Alias → resolves to i32, no new type
type Shape = Circle | Rectangle;   // Union → tagged union
type Pair = [f64, f64];           // Tuple → LLVM struct { double, double }
```

## Structural Typing

LLTS uses structural typing, matching TypeScript semantics. If two types have the same field layout, they're interchangeable:

```typescript
interface Point { x: f64; y: f64 }
type Coord = { x: f64; y: f64 };

function print(p: Point): void { /* ... */ }
const c: Coord = { x: 1, y: 2 };
print(c); // Valid — same shape, same LLVM struct
```

## Types Exist Only at Compile Time

No type metadata, no reflection, no RTTI. Runtime has:
- Integer tags (for union discrimination)
- Function pointers (for constructors / callbacks)
- Struct layouts (fixed at compile time)

`instanceof` compiles to an integer tag check on known union variants. `typeof` works for primitives (the compiler knows the type statically).
