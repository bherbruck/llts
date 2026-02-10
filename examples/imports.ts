// Imports — demonstrates importing functions and types from another file.

import { distance, add, Point } from "./math_lib";

function main(): void {
  const a: Point = { x: 0.0, y: 0.0 };
  const b: Point = { x: 3.0, y: 4.0 };
  const d: f64 = distance(a, b);
  print(d);

  const sum: f64 = add(10, 20);
  print(sum);
}
