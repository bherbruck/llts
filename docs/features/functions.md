# Functions & Closures

## Functions as Values

All function values — plain functions, closures, callbacks, struct fields — use the same fat pointer representation:

```typescript
// Function type: (a: i32, b: i32) => i32
// LLVM: { fn_ptr: ptr, env_ptr: ptr }

// Plain function — env_ptr is null
function add(a: i32, b: i32): i32 { return a + b; }
const f: (a: i32, b: i32) => i32 = add;
// → { fn_ptr: @add, env_ptr: null }

// Closure — env_ptr points to captured variables
const offset: i32 = 10;
const g: (a: i32) => i32 = (a) => a + offset;
// → { fn_ptr: @anon, env_ptr: &{offset: 10} }

// As a struct field — same representation
interface Handler {
  name: string;
  callback: (event: Event) => void;  // { fn_ptr, env_ptr }
}

// As a function argument — same representation
function apply(f: (x: i32) => i32, value: i32): i32 {
  return f(value);  // calls fn_ptr, passes env_ptr as hidden first arg
}
```

## Closures

**Non-escaping closures** (callbacks, `map`/`filter`): capture by reference from the stack. Zero cost.

**Escaping closures** (returned from functions, stored in objects): captured variables are automatically heap-allocated in a capture box (Swift-style). The closure holds a refcounted pointer to the box. Invisible to the developer.

```typescript
// Non-escaping: callback doesn't outlive forEach. Zero cost.
arr.forEach((x) => sum += x);

// Escaping: count is heap-allocated automatically
function makeCounter(): () => i32 {
  let count: i32 = 0;
  return () => { count += 1; return count; };
}
```

## Default Parameters

Sugar for a conditional at function entry:

```typescript
function greet(name: string = "world"): void { ... }
// → function greet(name: string): void { if (name === undefined) name = "world"; ... }
```

## Rest Parameters

Sugar for an array parameter. The caller constructs the array:

```typescript
function sum(...nums: i32[]): i32 { ... }
sum(1, 2, 3);
// → sum([1, 2, 3])
```

## Sugar Summary

| Source syntax | Compiles to |
|---|---|
| `class Foo { x: T; method() {} }` | Struct + free functions |
| `interface Foo { x: T }` | LLVM named struct (same as `type`) |
| `type Foo = { x: T }` | LLVM named struct (same as `interface`) |
| `new Foo(args)` | Constructor function returning struct |
| `obj.method(args)` | `Foo_method(obj, args)` |
| Arrow / regular function as value | Fat pointer `{ fn_ptr, env_ptr }` |
| Function as struct field | Fat pointer field |
| Function as argument | Fat pointer parameter |
