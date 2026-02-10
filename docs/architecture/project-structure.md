# Project Structure

## Workspace Layout

```
llts/
├── Cargo.toml                    # Workspace root
├── Cargo.lock
│
├── crates/
│   ├── llts_frontend/            # Parsing, semantic analysis, module resolution
│   │   ├── Cargo.toml            # deps: oxc_parser, oxc_ast, oxc_semantic, oxc_resolver, oxc_span, oxc_allocator
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── parse.rs          # oxc_parser wrapper, AST production
│   │       ├── semantic.rs       # oxc_semantic wrapper, scope/symbol resolution
│   │       └── resolve.rs        # Multi-file module resolution via oxc_resolver
│   │
│   ├── llts_analysis/            # Subset validation, type resolution, ownership
│   │   ├── Cargo.toml            # deps: llts_frontend, oxc_ast, oxc_span
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── validate.rs       # Compilable subset enforcement
│   │       ├── types.rs          # TS type → compiler IR type mapping
│   │       ├── ownership.rs      # Ownership/borrow tracking
│   │       ├── borrow.rs         # Borrow checker rules
│   │       └── monomorph.rs      # Generic monomorphization
│   │
│   ├── llts_codegen/             # LLVM IR generation (only crate with inkwell dep)
│   │   ├── Cargo.toml            # deps: llts_analysis, inkwell
│   │   └── src/
│   │       ├── lib.rs            # Main codegen driver (3-pass)
│   │       ├── types.rs          # LLVM type construction
│   │       ├── expr.rs           # Expression codegen
│   │       ├── stmt.rs           # Statement codegen
│   │       ├── call.rs           # Function call codegen
│   │       ├── narrowing.rs      # Type narrowing codegen (switch, instanceof, null checks)
│   │       ├── memory.rs         # Allocation, drops, ownership ops
│   │       └── intrinsics.rs     # Built-in functions (print, alloc, etc.)
│   │
│   ├── llts_driver/              # Pipeline orchestration
│   │   ├── Cargo.toml            # deps: llts_frontend, llts_analysis, llts_codegen
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pipeline.rs       # Full compilation pipeline
│   │       └── linker.rs         # Invoke system linker
│   │
│   └── llts_cli/                 # Binary entry point
│       ├── Cargo.toml            # deps: llts_driver, clap
│       └── src/
│           └── main.rs           # CLI arg parsing, invokes driver
│
├── std/                          # Standard library (.ts files)
│   ├── prelude.ts                # Option, Result, Vec, etc.
│   ├── io.ts
│   ├── string.ts
│   └── math.ts
│
├── tests/
│   ├── compile/                  # Should compile successfully
│   ├── error/                    # Should produce specific errors
│   └── run/                      # Compile + execute + verify output
│
└── examples/
    ├── hello.ts
    ├── fibonacci.ts
    ├── structs.ts
    └── ownership.ts
```

## Why a Workspace

**Compile times** — `llts_codegen` is the only crate that depends on inkwell/LLVM. Changing validation rules or the parser wrapper doesn't trigger an LLVM rebuild.

**Enforced boundaries** — Crate boundaries make the dependency graph explicit. `llts_frontend` can't accidentally depend on LLVM types. `llts_analysis` can't reach into codegen internals.

**Isolated testing** — Each crate has its own unit tests that run without compiling unrelated dependencies. `cargo test -p llts_analysis` is fast.

**Dependency graph:**
```
llts_cli → llts_driver → llts_codegen → llts_analysis → llts_frontend
                                                              ↓
                                                         oxc_* crates
```
