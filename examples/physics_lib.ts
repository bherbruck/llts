// Physics library — imports from types_lib.

import { Vec2, vec2_length } from "./types_lib";

export function velocity_magnitude(vel: Vec2): f64 {
  return vec2_length(vel);
}
