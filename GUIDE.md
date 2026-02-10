# TypeScript → Native Compiler Guide

A native compiler for a strict TypeScript subset using the oxc toolchain and LLVM via Inkwell.

---

## The Idea

Every TypeScript runtime (Bun, Deno, Node) throws away type information — they strip types and run JS. This compiler does the opposite: it treats types as real contracts and compiles TypeScript directly to native machine code via LLVM.

Developers write TypeScript they already know. The compiler enforces a strict, compilable subset and produces native binaries.

```
source.ts → oxc_parser → oxc_semantic → analysis → codegen → LLVM → native binary
```

Not a runtime. Not a transpiler. A compiler. Like Rust, Swift, or Zig — but the source language looks like TypeScript.

---

## Dependencies

```toml
[dependencies]
# Frontend (parsing, analysis, module resolution)
oxc_parser = "*"       # TS/JS parser → AST (fastest Rust parser, 100% Test262)
oxc_ast = "*"          # Typed AST node definitions
oxc_semantic = "*"     # Scope analysis, symbol resolution, bindings
oxc_resolver = "*"     # ESM/CJS module resolution, tsconfig paths
oxc_span = "*"         # Source locations, spans
oxc_allocator = "*"    # Arena allocator for AST nodes

# Backend (codegen)
inkwell = { version = "0.8.0", features = ["llvm21-1"] }
```

**oxc** handles everything about understanding TypeScript — parsing, type/scope resolution, imports. Fastest TS toolchain available.

**Inkwell/LLVM** handles everything about generating native code — IR, optimization, machine code for any target.

Our job is the bridge: deciding which TS patterns compile, mapping types, tracking ownership, and translating AST → LLVM IR.

---

## Key Design Decisions

### Memory: Invisible to the Developer

Hybrid stack + Lobster-style compile-time reference counting. No GC, no annotations. The compiler infers ownership, borrows, and moves — inserts refcounting only for the ~5% of cases where static analysis can't prove single ownership. → [memory-model.md](docs/features/memory-model.md)

### Types: Normal TypeScript + Ambient Numeric Types

`number` = f64. Specific types (`i32`, `u32`, `f64`, etc.) via ambient declarations in a prelude — valid TS, IDEs don't complain. Structural typing. `type` and `interface` compile to the same struct. → [type-system.md](docs/features/type-system.md) · [numeric-types.md](docs/features/numeric-types.md)

### Classes: Sugar for Structs + Functions

`class` = struct + free functions. `this` becomes the first parameter. `new Foo()` = constructor function. No vtable, no object header. → [classes.md](docs/features/classes.md)

### Functions: Universal Fat Pointer

All function values (plain, closures, callbacks, struct fields) use `{ fn_ptr, env_ptr }`. Non-escaping closures capture from the stack (zero cost). Escaping closures heap-allocate captures (Swift-style). → [functions.md](docs/features/functions.md)

### Errors: try/catch/throw → Result

`throw` compiles to returning `Err(...)`. `try`/`catch` compiles to `Result` branching. No LLVM exceptions, no stack unwinding. Familiar syntax, zero-cost implementation. → [error-handling.md](docs/features/error-handling.md)

### Narrowing: TS Patterns, Not Pattern Matching

Discriminated unions with `switch`/`if`, `instanceof` → tag checks, `T | null` → `Option<T>`. No custom `match` syntax. → [narrowing.md](docs/features/narrowing.md)

### What's Custom Beyond Normal TS

Very little:
- `i32`, `u32`, `f32`, etc. as types (via prelude, valid TS)
- `Weak<T>` for back-references in cyclic types (rare)
- `Readonly<T>` for immutable borrow contracts (existing TS utility type)

Everything else looks and feels like TypeScript.

---

## Documentation

### Architecture
- [Compilation Pipeline](docs/architecture/pipeline.md) — 7-stage pipeline from source to binary
- [Project Structure](docs/architecture/project-structure.md) — Cargo workspace layout
- [Desugaring](docs/architecture/desugaring.md) — What syntax sugar compiles to what

### Features (v1)
- [Memory Model](docs/features/memory-model.md) — ARC, escape analysis, ownership
- [Type System](docs/features/type-system.md) — Type mapping, structural typing, builtins
- [Numeric Types](docs/features/numeric-types.md) — Ambient declarations, literal inference
- [Classes](docs/features/classes.md) — Class → struct + functions
- [Functions & Closures](docs/features/functions.md) — Fat pointers, capture semantics
- [Generics](docs/features/generics.md) — Monomorphization, constraints, defaults
- [Enums](docs/features/enums.md) — Numeric, string, const enums
- [Unions](docs/features/unions.md) — Discriminated, numeric, string literal, nullable
- [Error Handling](docs/features/error-handling.md) — try/catch → Result
- [Type Narrowing](docs/features/narrowing.md) — Discriminated unions, instanceof, type guards
- [Iterators](docs/features/iterators.md) — for...of on arrays
- [Modules](docs/features/modules.md) — Single compilation unit
- [Standard Library](docs/features/stdlib.md) — Three layers, strings, I/O, C FFI

### v2 (Stubbed)
- [Async/Await](docs/v2/async.md) — State machine transform
- [Generators](docs/v2/generators.md) — State machine transform
- [Collections](docs/v2/collections.md) — Map, Set

### Rejected
- [Decorators](docs/rejected/decorators.md) — Runtime metaprogramming, can't compile statically

---

## Linting / Validation Tooling

Oxlint (the oxc linter) can run alongside the compiler to provide IDE feedback. Custom oxlint rules can enforce the compilable subset in the editor before compilation, giving developers instant feedback on what will and won't compile.
