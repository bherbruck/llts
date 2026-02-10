# Classes

A `class` compiles to a struct + free functions. No vtable, no hidden object header.

```typescript
class Point {
  x: f64;
  y: f64;
  constructor(x: f64, y: f64) { this.x = x; this.y = y; }
  distance(other: Point): f64 {
    return Math.sqrt((this.x - other.x) ** 2 + (this.y - other.y) ** 2);
  }
}

// Compiles identically to:
interface Point { x: f64; y: f64 }
function Point_new(x: f64, y: f64): Point { return { x, y }; }
function Point_distance(self: Point, other: Point): f64 {
  return Math.sqrt((self.x - other.x) ** 2 + (self.y - other.y) ** 2);
}
```

## How It Maps

- `new Point(1, 2)` → `Point_new(1, 2)` (constructor = regular function returning a struct)
- `p.distance(other)` → `Point_distance(p, other)` (method = free function, `this` becomes first arg)
- Class fields → struct fields (same layout as an equivalent `interface`)

## Getters / Setters

Sugar for function calls:

```typescript
class Foo {
  private _x: i32 = 0;
  get x(): i32 { return this._x; }
  set x(v: i32) { this._x = v; }
}
foo.x;     // → Foo_get_x(foo)
foo.x = 5; // → Foo_set_x(foo, 5)
```

Desugared in `llts_analysis`, codegen sees regular function calls.
