// Structs — demonstrates interfaces compiling to structs.

interface Point {
  x: f64;
  y: f64;
}

function distance(a: Point, b: Point): f64 {
  const dx: f64 = a.x - b.x;
  const dy: f64 = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}

function main(): void {
  const a: Point = { x: 0.0, y: 0.0 };
  const b: Point = { x: 3.0, y: 4.0 };
  const d: f64 = distance(a, b);
  print(d);
}
