# Unions

## Tagged Unions (Discriminated)

The primary union pattern in LLTS. A discriminant field (typically `kind` or `type`) maps to an integer tag at compile time.

```typescript
interface Circle { kind: "circle"; radius: f64 }
interface Rectangle { kind: "rectangle"; width: f64; height: f64 }
type Shape = Circle | Rectangle;
```

### LLVM Layout

```
Shape = { i32_tag, [max_variant_payload] }
```

- Tag 0 = Circle, Tag 1 = Rectangle
- Payload sized to largest variant (Rectangle: `{ f64, f64 }` = 16 bytes)
- The `kind` string field is **not stored at runtime** — it's represented by the tag
- Smaller variants (Circle: `{ f64 }`) use only part of the payload slot

### Construction

```typescript
let s: Shape = { kind: "circle", radius: 5 };
// Compiles to: build_union_value(tag=0, payload={radius: 5.0})
```

The compiler:
1. Identifies the discriminant field (`kind`) and its value (`"circle"`)
2. Maps `"circle"` → tag 0
3. Strips the discriminant field from the payload struct
4. Calls `build_union_value` with the tag and payload

### Narrowing (Extraction)

```typescript
switch (shape.kind) {
  case "circle":
    return 3.14159 * shape.radius ** 2;    // shape is Circle here
  case "rectangle":
    return shape.width * shape.height;      // shape is Rectangle here
}
```

Compiles to:
1. `build_discriminant_switch` — switch on tag
2. In each case block: `build_union_extract` to get the typed payload
3. Access fields from the extracted struct

### Discriminant Detection

The compiler identifies the discriminant field by:
1. All variants are struct types (interfaces/type literals)
2. All variants share a field with the same name
3. That field's type is a **string literal type** in each variant
4. Each variant has a unique literal value for that field

Common discriminant names: `kind`, `type`, `tag`, `_tag`, `discriminator`

## Primitive Unions

```typescript
type SensorValue = i8 | i32 | f32;
```

Layout: `{ i32_tag, f32 }` — payload sized to largest (f32 = 4 bytes).

Construction requires explicit type context:
```typescript
let v: SensorValue = 42 as i32;  // tag=1 (i32 variant)
```

## String Literal Unions (Enum-like)

```typescript
type Status = "pending" | "active" | "done";
```

Compiles to a **plain integer enum** — no payload, no string storage:
```
"pending" = 0, "active" = 1, "done" = 2
```

Layout: just `i32`. Comparison is integer comparison. The strings exist only at compile time for type checking.

## Nullable Types (`T | null`)

```typescript
type MaybeUser = User | null;
// or equivalently:
let user: User | null = null;
```

Compiles to `Option<T>` → `{ i1_tag, T }`:
- Tag 0 = None (null)
- Tag 1 = Some (value present)

Null pointer optimization: if T is a pointer type (string, array, struct by reference), the option is represented as a nullable pointer — no tag overhead.

## Numeric Unions (Auto-Widening)

When all variants of a union are numeric types, the union is **automatically widened** to the largest type. No tag, no union struct, no overhead.

```typescript
type SensorValue = i8 | i32 | f64;
// Compiles to: f64 (widened to largest)

type SmallInt = i8 | i32;
// Compiles to: i32

type AnyFloat = f32 | f64;
// Compiles to: f64
```

Widening rules (same as implicit widening in binary ops):
- Any int + any float → widest float (`f64` if either is `f64`)
- All ints → widest int
- All floats → widest float
- Signed + unsigned of same width → signed (to preserve sign info)

This matches TypeScript semantics where all numbers are `number` (f64) anyway — LLTS just picks the most efficient concrete type that can represent all variants.

## Mixed-Type Unions (Auto-Tagged)

Unions of incompatible types are **automatically tagged** — no discriminant field needed:

```typescript
type Value = string | i32;
// Compiles to: { i32_tag, [16 bytes] }  (sized to string, the largest)
// tag 0 = string, tag 1 = i32
```

Narrowing via `typeof`:

```typescript
function process(value: string | i32): void {
  if (typeof value === "string") {
    print(value);           // narrowed to string
  } else {
    print(value + 1);       // narrowed to i32
  }
}
// typeof compiles to a tag check — compiler knows tag 0 = string, tag 1 = i32
```

Any combination of types works — the payload is always sized to the largest variant:

```typescript
type Anything = string | i32 | boolean | Vec2;
// tag 0=string (16b), 1=i32 (4b), 2=boolean (1b), 3=Vec2 (16b)
// payload = 16 bytes (max of all variants)
```

### Narrowing Patterns for Mixed Unions

| Pattern | Compiles to |
|---------|-------------|
| `typeof x === "string"` | Tag check (`tag == 0`) |
| `typeof x === "number"` | Tag check (matches any numeric variant) |
| `typeof x === "boolean"` | Tag check |
| `x instanceof ClassName` | Tag check for that class's variant index |
| Type guard function | User-defined tag check |

## Anonymous Structs in Unions

Inline object types in unions are compiled as **anonymous structs** — no named `interface` required. The compiler synthesizes a struct name from the type context.

### Inline Object Types

```typescript
type Response = { data: string; error: null } | { data: null; error: string };
```

The compiler:
1. Encounters two anonymous object types in the union
2. Synthesizes names: `Response$0` and `Response$1` (union name + variant index)
3. Lowers each to a regular struct definition
4. Proceeds with discriminated union logic if a discriminant field exists, or auto-tagged union otherwise

Layout (auto-tagged — no shared discriminant):
```
Response = { i32_tag, [24 bytes] }
// tag 0 = Response$0 { data: string, error: null }  → { {ptr,len}, i1 } = 17 bytes
// tag 1 = Response$1 { data: null, error: string }   → { i1, {ptr,len} } = 17 bytes
// payload = 24 bytes (padded max)
```

### Inline Objects as Discriminated Variants

Anonymous object types work with discriminated unions too:

```typescript
type Event =
  | { type: "click"; x: f64; y: f64 }
  | { type: "key"; code: i32 };
```

Compiles identically to named interfaces — the compiler detects the `type` discriminant field across the anonymous variants.

### Inline Object Parameters

Anonymous struct types in function parameters:

```typescript
function area(rect: { width: f64; height: f64 }): f64 {
  return rect.width * rect.height;
}
```

The compiler synthesizes `area$rect` as the struct name (function name + parameter name). Called with an object literal:

```typescript
let a = area({ width: 3.0, height: 4.0 });
// Compiles to: area$rect { width: 3.0, height: 4.0 }
```

### Untyped Object Literals

When an object literal has no type annotation and isn't assignable to a known struct:

```typescript
let point = { x: 1.0, y: 2.0 };
// Compiler infers struct type from fields: { x: f64, y: f64 }
// Synthesized name: __anon$x_f64$y_f64 (field names + types, deterministic)
```

Identical anonymous structs are deduplicated — two `{ x: f64, y: f64 }` literals at different locations share the same synthesized struct type.

### Name Synthesis Rules

| Context | Synthesized Name | Example |
|---------|-----------------|---------|
| Union variant | `{UnionName}${index}` | `Response$0` |
| Function parameter | `{FnName}${ParamName}` | `area$rect` |
| Variable declaration | `{VarName}$type` | `point$type` |
| Untyped literal | `__anon${field_signature}` | `__anon$x_f64$y_f64` |

All synthesized names use `$` separator (valid in LLVM, invalid in TypeScript — no collisions).

## Implementation Plan

### Phase 1: Discriminated Union Wiring

**Where:** `llts_driver/src/pipeline.rs`

1. **Detect discriminant pattern** — When lowering a `TSUnionType` where all variants are object types sharing a string-literal field, identify it as a discriminated union.

2. **Build variant map** — Map each discriminant value to a tag index and a payload struct type (with the discriminant field stripped).

3. **Store in LowerCtx** — New field: `discriminated_unions: HashMap<String, DiscriminatedUnionDef>` containing tag mappings, variant types, discriminant field name.

4. **Lower construction** — When lowering an object literal assigned to a discriminated union type, detect the discriminant value, look up the tag, emit `Expr::UnionLit { tag, payload }`.

5. **Lower switch narrowing** — When lowering `switch (x.kind)`, detect that `x` is a discriminated union and `kind` is the discriminant. Emit `Stmt::Switch` with tag comparisons. In each case, narrow the variable type to the specific variant.

### Phase 2: String Literal Unions

1. **Detect in `lower_ts_type`** — When a `TSUnionType` contains only `TSLiteralType(StringLiteral)` variants, emit `LltsType::I32` (enum-like).

2. **Build string→int map** — Store in LowerCtx for comparison lowering.

3. **Lower comparisons** — `status === "active"` compiles to `status == 1`.

### Phase 3: Null Unions (Option)

1. **Already partially handled** — `T | null` → `Option<T>` in `lower_ts_type`.
2. **Wire up narrowing** — `if (x !== null)` → `build_option_is_some` + branch.
3. **Wire up construction** — `null` → `build_option_none`, value → `build_option_some`.
4. **Wire up extraction** — After null check, `build_option_unwrap` to get inner value.

### Codegen Support (Already Exists)

All codegen building blocks are implemented in `narrowing.rs`:
- `build_discriminant_switch` — switch on union tag
- `build_instanceof_check` — compare tag to expected
- `build_union_value` / `build_union_extract` — construct/extract payloads
- `build_option_some/none/unwrap/is_some/is_none` — Option operations
- `build_option_narrow` — if-let narrowing pattern
