// Math library — demonstrates exporting functions and types.

export interface Point {
  x: f64;
  y: f64;
}

export function distance(a: Point, b: Point): f64 {
  const dx: f64 = a.x - b.x;
  const dy: f64 = a.y - b.y;
  return Math.sqrt(dx * dx + dy * dy);
}

export function add(a: f64, b: f64): f64 {
  return a + b;
}
