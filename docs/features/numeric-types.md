# Numeric Types

## Ambient Declarations (AssemblyScript Pattern)

`number` stays as f64 (JS semantics). Specific numeric types are declared in a compiler prelude as ambient type aliases — valid TypeScript that the compiler recognizes as distinct LLVM types:

```typescript
// prelude.d.ts (ships with the compiler, valid TS for IDE compatibility)
declare type i8 = number;
declare type i16 = number;
declare type i32 = number;
declare type i64 = number;
declare type u8 = number;
declare type u16 = number;
declare type u32 = number;
declare type u64 = number;
declare type f32 = number;
declare type f64 = number;
```

## Literal Inference for `const`

- `const x = 5` → `i32` (no fractional part, fits in 32 bits)
- `const x = 5.5` → `f64`
- `const x = 5_000_000_000` → `i64` (exceeds i32 range)
- `let x = 5` → `f64` (mutable binding, safe default for JS compat)
- `const x: number = 5` → `f64` (explicit `number` = f64)
- `const x: i32 = 5` → `i32` (explicit annotation wins)

## Arithmetic Type Promotion

- Implicit widening (no cast needed): `i8` → `i16` → `i32` → `i64`, `f32` → `f64`, any int → `f64`
- Explicit narrowing (cast required): `f64` → `i32`, `i64` → `i32` (potential data loss)
- Mixed `i32 + f64`: auto-promote the `i32` to `f64`, result is `f64` (matches JS behavior)
- Explicit casts: `value as i32` (same syntax as TypeScript type assertions)
