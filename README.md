# LLTS — TypeScript to Native Compiler

LLTS compiles a strict TypeScript subset directly to native machine code via LLVM. Unlike TypeScript runtimes that strip types and run JavaScript, LLTS treats types as real contracts and produces native binaries — like Rust, Swift, or Zig, but the source looks like TypeScript.

```
source.ts → oxc_parser → analysis → LLVM IR → native binary
```

## Install

Requires LLVM 21 and a C linker (cc/gcc/clang).

```bash
# Ubuntu/Debian — install LLVM 21
wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key | sudo apt-key add -
echo "deb http://apt.llvm.org/$(lsb_release -cs)/ llvm-toolchain-$(lsb_release -cs) main" | sudo tee /etc/apt/sources.list.d/llvm.list
sudo apt-get update && sudo apt-get install -y llvm-21-dev

# macOS
brew install llvm@21

# Install LLTS
cargo install llts
```

## Quick Start

### Hello World

```typescript
// hello.ts
function main(): void {
  print("Hello, World!");
}
```

```bash
# Compile to native binary
llts hello.ts

# Or compile and run immediately
llts hello.ts --run
```

## Usage

```
llts [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input TypeScript file to compile

Options:
  -o, --output <OUTPUT>      Output file path [default: build/<name>]
  -O, --opt-level <0-3>      Optimization level [default: 2]
      --emit-ir              Emit LLVM IR text instead of a binary
  -r, --run                  Compile and run immediately (temp binary cleaned up)
```

## Language Features

LLTS supports a compilable subset of TypeScript. Everything looks like normal TypeScript — your IDE, linter, and formatter work as usual.

### Numeric Types

Beyond `number` (f64), LLTS provides fixed-width numeric types via ambient declarations:

```typescript
let a: i32 = 42;        // 32-bit signed integer
let b: u64 = 100;       // 64-bit unsigned integer
let c: f32 = 3.14;      // 32-bit float
let d: f64 = 2.718;     // 64-bit float (same as `number`)
```

Full set: `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, `f64`

Implicit widening is supported — `i8` promotes to `i32`, integers promote to floats when mixed.

### Structs (Interfaces & Types)

Both `interface` and `type` compile to LLVM structs:

```typescript
interface Point {
  x: f64;
  y: f64;
}

function distance(a: Point, b: Point): f64 {
  const dx: f64 = a.x - b.x;
  const dy: f64 = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}
```

### Classes

Classes compile to structs + free functions. No vtable, no object header:

```typescript
class Vector2 {
  x: f64;
  y: f64;

  constructor(x: f64, y: f64) {
    this.x = x;
    this.y = y;
  }

  length(): f64 {
    return Math.sqrt(this.x * this.x + this.y * this.y);
  }
}

function main(): void {
  const v = new Vector2(3.0, 4.0);
  print(v.length()); // 5
}
```

### Functions & Closures

All function values use a universal fat pointer `{ fn_ptr, env_ptr }`. Non-escaping closures capture from the stack at zero cost:

```typescript
function apply(f: (x: f64) => f64, val: f64): f64 {
  return f(val);
}

function main(): void {
  const scale: f64 = 2.0;
  const result = apply((x: f64): f64 => x * scale, 5.0);
  print(result); // 10
}
```

### Generics

Generics are monomorphized at compile time (like Rust/C++). Each concrete instantiation produces specialized machine code:

```typescript
function identity<T>(x: T): T {
  return x;
}

function add<T extends i32 | f64>(a: T, b: T): T {
  return a + b;
}

function main(): void {
  let a: i32 = identity<i32>(42 as i32);
  let b: f64 = add<f64>(1.5, 2.5);
  print(a);
  print(b);
}
```

Supports constraints (`extends`), default type parameters (`<T = f64>`), and type alias constraints.

### Enums

Enums compile to integer constants — fully inlined, no runtime object:

```typescript
enum Direction {
  Up,
  Down,
  Left,
  Right,
}

enum Color {
  Red = 10,
  Green = 20,
  Blue = 30,
}

function main(): void {
  let d: Direction = Direction.Up;
  print(d); // 0
}
```

String enums are also supported — string values become compile-time-only, stored as integer tags at runtime.

### Unions

**Nullable types** — `T | null` compiles to `Option<T>`:

```typescript
function find(arr: f64[], target: f64): f64 | null {
  for (let i: i32 = 0 as i32; i < arr.length; i++) {
    if (arr[i] === target) return arr[i];
  }
  return null;
}
```

**Numeric unions** auto-widen to the largest type:

```typescript
let x: i8 | i32 | f64 = 42; // stored as f64
```

**String literal unions** compile to integer enums:

```typescript
type Status = "active" | "inactive" | "pending";
let s: Status = "active"; // stored as i32 (0)
```

**Discriminated unions** with switch narrowing:

```typescript
type Shape =
  | { kind: "circle"; radius: f64 }
  | { kind: "rect"; width: f64; height: f64 };

function area(s: Shape): f64 {
  switch (s.kind) {
    case "circle": return 3.14159 * s.radius * s.radius;
    case "rect":   return s.width * s.height;
  }
}
```

### Error Handling

`try/catch/throw` compiles to `Result<T, E>` branching — no LLVM exceptions, no stack unwinding:

```typescript
function divide(a: f64, b: f64): f64 {
  if (b === 0) throw "division by zero";
  return a / b;
}

function main(): void {
  try {
    print(divide(10, 0));
  } catch (e) {
    print(e);
  }
}
```

### Arrays

```typescript
function main(): void {
  let nums: i32[] = [1 as i32, 2 as i32, 3 as i32];
  for (const n of nums) {
    print(n);
  }
}
```

### Imports

Multi-file programs with ES module imports:

```typescript
// math.ts
export function add(a: f64, b: f64): f64 {
  return a + b;
}

// main.ts
import { add } from "./math";

function main(): void {
  print(add(1.0, 2.0));
}
```

### Math Intrinsics

`Math.sqrt`, `Math.abs`, `Math.floor`, `Math.ceil`, `Math.round`, `Math.min`, `Math.max`, `Math.pow`, `Math.log`, `Math.sin`, `Math.cos` compile to LLVM intrinsics.

## How It Works

### Compilation Pipeline

| Stage | Tool | Purpose |
|-------|------|---------|
| 1. Parse | oxc_parser | TypeScript source → typed AST |
| 2. Semantic Analysis | oxc_semantic | Scopes, symbols, bindings |
| 3. Module Resolution | oxc_resolver | Resolve import paths |
| 4. Subset Validation | llts_analysis | Enforce compilable TS rules |
| 5. Type Resolution | llts_driver | Map TS types → LLVM types |
| 6. Code Generation | llts_codegen | AST → LLVM IR (via Inkwell) |
| 7. Optimization & Emit | LLVM | Passes → object file → linker |

### Memory Model

No garbage collector. No manual memory management. The compiler uses a three-tier strategy:

1. **Stack allocation** — default for primitives and small structs
2. **Escape analysis** — heap promotion only when values escape their scope
3. **Compile-time reference counting** — Lobster-style ARC eliminates ~95% of refcount operations at compile time

This is invisible to the developer — no ownership annotations, no lifetime syntax.

### What's Not Supported

LLTS rejects patterns that can't compile statically:

- `any` / `unknown` without narrowing
- Dynamic property access (`obj[expr]`)
- `eval`, `with`, `Proxy`, `Reflect`
- Prototype manipulation
- Decorators
- async/await (planned for v2)

## Project Structure

```
llts/
├── crates/
│   ├── llts_frontend/     # oxc parser + semantic + resolver wrappers
│   ├── llts_analysis/     # Subset validation, type resolution
│   ├── llts_codegen/      # LLVM IR generation via Inkwell
│   ├── llts_driver/       # Pipeline orchestration
│   └── llts/          # CLI binary
├── std/prelude.ts         # Ambient type declarations (i32, Option, etc.)
├── docs/                  # Architecture & feature documentation
├── tests/run/             # Compile + execute test suite
├── examples/              # Example programs (including a multi-file game)
├── benchmarks/            # Performance benchmarks (LLTS vs Bun)
└── editor/vscode/         # VS Code extension with TypeScript plugin
```

## Benchmarks

Run the benchmark suite comparing LLTS (native, -O2) against Bun:

```bash
bash benchmarks/run.sh
```

Benchmarks include: fibonacci, mandelbrot, leibniz pi, n-body, sieve of eratosthenes, spectral norm, ackermann, and euler sum.

## VS Code Extension

The `editor/vscode/llts/` directory contains a VS Code extension that provides IDE support for LLTS prelude types (`i32`, `u64`, `Option<T>`, etc.) via a TypeScript language service plugin.

## Documentation

Detailed documentation is available in the `docs/` directory:

**Architecture:**
[Pipeline](docs/architecture/pipeline.md) | [Project Structure](docs/architecture/project-structure.md) | [Desugaring](docs/architecture/desugaring.md)

**Features:**
[Type System](docs/features/type-system.md) | [Numeric Types](docs/features/numeric-types.md) | [Memory Model](docs/features/memory-model.md) | [Classes](docs/features/classes.md) | [Functions](docs/features/functions.md) | [Generics](docs/features/generics.md) | [Enums](docs/features/enums.md) | [Unions](docs/features/unions.md) | [Error Handling](docs/features/error-handling.md) | [Narrowing](docs/features/narrowing.md) | [Iterators](docs/features/iterators.md) | [Modules](docs/features/modules.md) | [Standard Library](docs/features/stdlib.md)

**Planned (v2):**
[Async/Await](docs/v2/async.md) | [Generators](docs/v2/generators.md) | [Collections](docs/v2/collections.md)

See also: [GUIDE.md](GUIDE.md) for design philosophy and key decisions.

## License

MIT
