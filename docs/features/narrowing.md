# Type Narrowing

TypeScript doesn't have pattern matching — it has **narrowing**. Developers use `switch`, `if`, `instanceof`, and discriminant fields to narrow union types. We compile these directly instead of inventing new syntax.

## Discriminated Unions (Primary Pattern)

```typescript
type Shape =
  | { kind: "circle"; radius: f64 }
  | { kind: "rectangle"; width: f64; height: f64 };

function area(shape: Shape): f64 {
  switch (shape.kind) {
    case "circle": return Math.PI * shape.radius ** 2;
    case "rectangle": return shape.width * shape.height;
  }
}
// Compiles to: switch on integer tag (0 = circle, 1 = rectangle)
// String discriminants are mapped to integer tags at compile time
// Exhaustiveness is checked — missing a case is a compile error
```

## `instanceof` on Class Unions (Tag Check)

```typescript
try { foo(); } catch (e) {
  if (e instanceof TypeError) { ... }   // → if (tag == 0)
  if (e instanceof RangeError) { ... }  // → if (tag == 1)
}
```

Also works with `switch(true)` — a valid TS pattern:

```typescript
switch (true) {
  case error instanceof TypeError: handleType(error); break;
  case error instanceof RangeError: handleRange(error); break;
}
```

## `T | null` Narrowing (Null Check)

```typescript
function getName(user: User | null): string {
  if (user === null) return "anonymous";
  return user.name;  // compiler knows user is User here
}
// Compiles to: Option<User> check — branch on tag or null pointer
```

## Type Guards

Compile-time narrowing only. The guard function executes normally; the `s is Circle` return type annotation tells the compiler to narrow the type in the caller:

```typescript
function isCircle(s: Shape): s is Circle {
  return s.kind === "circle";
}
if (isCircle(shape)) {
  // compiler knows shape is Circle here — same as checking shape.kind directly
}
```

## No Runtime Type Metadata

Types exist only at compile time. `instanceof` compiles to an integer tag check on known union variants, not RTTI. `typeof` works for primitives (the compiler knows the type statically). No reflection.
