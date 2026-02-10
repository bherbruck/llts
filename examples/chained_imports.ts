// Chained imports — A imports B, B imports C.

import { velocity_magnitude } from "./physics_lib";
import { Vec2 } from "./types_lib";

function main(): void {
  const v: Vec2 = { x: 3.0, y: 4.0 };
  const speed: f64 = velocity_magnitude(v);
  print(speed);
}
