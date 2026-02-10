# Standard Library

## Three Layers

### Layer 1: Compiler Intrinsics (no TS source, maps directly to LLVM/libc)

- Arithmetic, bitwise, comparison operators → LLVM instructions
- `Math.sqrt`, `Math.abs`, `Math.floor`, `Math.ceil` → LLVM intrinsics
- `Math.sin`, `Math.cos`, `Math.log`, `Math.exp`, `Math.pow` → libc calls
- Memory allocation → libc `malloc`/`free`

### Layer 2: Core Types (written in the compilable TS subset)

- `String` → `{ ptr: Ptr<u8>, len: usize }` fat pointer to UTF-8 data. Refcounted, small-string optimization for ≤23 bytes, copy-on-write for slicing.
- `Array<T>` → `{ ptr: Ptr<T>, len: usize, cap: usize }` heap-allocated, growable (like Rust's Vec).
- `Option<T>` → tagged union. Null pointer optimization for pointer types.
- `Result<T, E>` → tagged union. Replaces exceptions.

### Layer 3: I/O (thin wrappers around libc)

- `console.log()` → format args + libc `write(1, ptr, len)`
- Number formatting → libc `snprintf`
- File I/O → wrappers around `open`/`read`/`write`/`close`

## Strings

UTF-8 (not UTF-16 like JS/AssemblyScript) because libc, Linux, Rust, Go, Zig, and LLVM all use UTF-8. No conversion needed for I/O or C interop.

## C FFI

Compiled LLTS can call C libraries using TypeScript's existing `declare` syntax:

```typescript
declare function puts(s: string): i32;  // libc
declare function sqrt(x: f64): f64;     // libm
```

The compiler maps these to external LLVM function declarations linked at the system linker stage.

## Minimum Viable Stdlib (Hello World)

String type, string literals, `print()`, `main()` entry point.
