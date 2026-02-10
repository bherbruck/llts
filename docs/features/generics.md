# Generics (Monomorphization)

Generics compile via monomorphization — the compiler stamps out a specialized copy of each generic function/type for every concrete type it's used with. No boxing, no vtables, no runtime cost.

## Syntax

Standard TypeScript generic syntax:

```typescript
function identity<T>(x: T): T {
  return x;
}

let a: i32 = identity<i32>(5 as i32);   // → identity$i32(i32) -> i32
let b: f64 = identity<f64>(3.14);       // → identity$f64(f64) -> f64
```

## Generic Interfaces / Types

```typescript
interface Pair<A, B> {
  first: A;
  second: B;
}

let p: Pair<i32, string> = { first: 42 as i32, second: "hello" };
// → %struct.Pair$i32$string = type { i32, { ptr, i64 } }
```

## Constraints

Type constraints via `extends`:

```typescript
interface HasLength {
  length: f64;
}

function getLength<T extends HasLength>(x: T): f64 {
  return x.length;
}
```

Constraints are checked at the call site — the concrete type must satisfy the constraint structurally.

## Built-in Generic Types

These are the first consumers of monomorphization:

| Type | Layout | Notes |
|------|--------|-------|
| `Array<T>` | `{ ptr, len, cap }` | Already implemented as `T[]` |
| `Option<T>` | `{ i1, T }` | `T \| null` sugar |
| `Result<T, E>` | `{ i32, union(T, E) }` | Tagged union |
| `Map<K, V>` | Hash map (v2) | Requires hashable K |
| `Set<T>` | Hash set (v2) | Requires hashable T |
| `Readonly<T>` | Same layout as T | Immutable borrow contract |

## Implementation Plan

### Phase 1: Generic Function Monomorphization

**Where:** `llts_analysis/src/monomorph.rs` (new) + `llts_driver/src/pipeline.rs`

1. **Detect generic definitions** — In Pass 1 of `lower_program_with_ctx`, when encountering a function with `TSTypeParameterDeclaration`, store it in a new `LowerCtx.generic_fn_defs` map instead of lowering it immediately. Key: function name, value: the raw oxc AST node.

2. **Detect call sites** — In Pass 3, when lowering a `CallExpression` where the callee is a known generic function:
   - Extract concrete type arguments from `TSTypeParameterInstantiation` (e.g. `<i32>`)
   - If no explicit type args, infer from argument types
   - Compute a mangled name: `identity$i32`

3. **Stamp out specialization** — If `identity$i32` hasn't been generated yet:
   - Clone the generic function's AST
   - Substitute `T → i32` in all type annotations
   - Lower the specialized copy as a normal function
   - Register it in `LowerCtx.fn_ret_types` and the `ProgramIR.functions`

4. **Rewrite call site** — Replace the call to `identity<i32>(x)` with a call to `identity$i32(x)`

### Phase 2: Generic Interface/Type Monomorphization

Same approach but for struct types:

1. Store generic interface definitions in `LowerCtx.generic_type_defs`
2. When `Pair<i32, string>` is referenced, stamp out `Pair$i32$string` as a concrete struct
3. Register in `LowerCtx.struct_defs`

### Phase 3: Constraint Checking

1. When specializing `getLength<Vec2>`, verify Vec2 has a `length` field
2. Structural check — does the concrete type satisfy the `extends` constraint?
3. Emit a clear compile error if not

### Name Mangling

```
identity<i32>           → identity$i32
identity<f64>           → identity$f64
Pair<i32, string>       → Pair$i32$string
Map<string, Array<i32>> → Map$string$Array$i32
```

`$` separator chosen because it's valid in LLVM symbol names but not in TypeScript identifiers, so no collisions.

### Edge Cases

- **Recursive generics** — `function foo<T>(x: T): T { return foo<T>(x); }` — depth limit (e.g. 64)
- **Unused generics** — Only instantiated specializations are emitted (dead code elimination for free)
- **Cross-module generics** — Generic defs from imported files are stored in the shared `LowerCtx`, available for specialization in any file
- **Default type parameters** — `function foo<T = f64>(x: T)` — use default when no type arg provided
