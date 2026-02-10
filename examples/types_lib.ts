// Types library — base types used by other modules.

export interface Vec2 {
  x: f64;
  y: f64;
}

export function vec2_length(v: Vec2): f64 {
  return Math.sqrt(v.x * v.x + v.y * v.y);
}
