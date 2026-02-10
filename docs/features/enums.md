# Enums

Enums compile to integer constants. Every enum member gets an `i32` value at compile time — no runtime enum object is generated. All member references are inlined to their integer literal at every use site.

```typescript
enum Direction { Up, Down, Left, Right }
// Up = 0, Down = 1, Left = 2, Right = 3

let d: Direction = Direction.Left;
// Compiles to: let d: i32 = 2
```

## Numeric Enums

Members without an initializer auto-increment from 0 (or from the last explicit value):

```typescript
enum Basic { A, B, C }          // A = 0, B = 1, C = 2
enum Explicit { X = 10, Y = 20 } // X = 10, Y = 20
enum Mixed { A = 5, B, C }       // A = 5, B = 6, C = 7
```

Every reference to a member compiles to the literal integer:

```typescript
let x = Explicit.Y;
// Compiles to: let x: i32 = 20
```

### LLVM Layout

No layout — enum members are not stored in memory. Each use site is replaced with an `i32` constant:

```
Direction.Up   → i32 0
Direction.Down → i32 1
Mixed.C        → i32 7
```

## String Enums

String-initialized members are assigned integer tags (0, 1, 2, ...) based on declaration order. The string values exist only at compile time for type checking — they are never stored at runtime.

```typescript
enum Color { Red = "RED", Green = "GREEN", Blue = "BLUE" }
// Red = 0, Green = 1, Blue = 2  (strings are compile-time only)

let c = Color.Red;
// Compiles to: let c: i32 = 0
```

Comparisons against enum members compile to integer comparisons:

```typescript
if (c === Color.Green) { ... }
// Compiles to: if (c == 1) { ... }
```

## Const Enums

In standard TypeScript, `const enum` is a hint to inline values and omit the runtime enum object. In LLTS, **all enums are already inlined** — no runtime enum object is ever generated. The `const` modifier is accepted but has no additional effect.

```typescript
const enum Flags { None = 0, Read = 1, Write = 2 }

let f = Flags.Write;
// Compiles to: let f: i32 = 2
// (identical to a regular enum — both are fully inlined)
```

## Enum as a Type

Enum names can be used as types in function parameters, return types, and variable declarations. They compile to `i32`.

```typescript
enum Color { Red = "RED", Green = "GREEN", Blue = "BLUE" }

function paint(c: Color): void {
  // c is i32 at runtime
}

paint(Color.Red);
// Compiles to: paint(0)
```

Function signature in LLVM IR:

```
define void @paint(i32 %c) { ... }
```

## Relationship to String Literal Unions

String literal union types (documented in [unions.md](unions.md#string-literal-unions-enum-like)) are the idiomatic LLTS alternative to string enums and compile the same way — to `i32` tags:

```typescript
// String enum approach:
enum Status { Pending = "PENDING", Active = "ACTIVE", Done = "DONE" }

// String literal union approach (preferred):
type Status = "pending" | "active" | "done";
```

Both compile to `i32`. The string literal union form is often preferred because:
- No extra declaration — the type is self-describing
- Works with discriminated unions and `switch` narrowing
- Matches idiomatic TypeScript patterns

## Not Supported

| Feature | Status | Notes |
|---------|--------|-------|
| Reverse mapping (`Color[0]`) | Not supported | No runtime enum object exists to index into |
| Computed members (`A = 1 + 2`) | Not supported | Initializers must be integer or string literals |
| Heterogeneous enums (mixed numeric + string) | Not supported | All members must be the same kind |
| Ambient / `declare enum` | Not supported | No declaration merging |
| Bitwise flag patterns (`A \| B`) | Works manually | Use explicit powers of 2 and bitwise ops on the `i32` values |
