// Ownership — demonstrates how the compiler manages memory automatically.

interface Vec2 {
  x: f64;
  y: f64;
}

// Compiler infers: a and b are immutable borrows (no RC, pass as pointer)
function add(a: Vec2, b: Vec2): Vec2 {
  return { x: a.x + b.x, y: a.y + b.y };
}

// Compiler infers: v is moved (stored in a longer-lived collection)
function scale(v: Vec2, s: f64): Vec2 {
  return { x: v.x * s, y: v.y * s };
}

function main(): void {
  const a: Vec2 = { x: 1.0, y: 2.0 };
  const b: Vec2 = { x: 3.0, y: 4.0 };
  const sum: Vec2 = add(a, b);
  const scaled: Vec2 = scale(sum, 2.0);
  print(scaled.x);
  print(scaled.y);
}
