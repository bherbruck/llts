// Vec2 — 2D vector type and basic operations.

export interface Vec2 {
  x: number;
  y: number;
}

export function vec2_add(a: Vec2, b: Vec2): Vec2 {
  return { x: a.x + b.x, y: a.y + b.y };
}

export function vec2_sub(a: Vec2, b: Vec2): Vec2 {
  return { x: a.x - b.x, y: a.y - b.y };
}

export function vec2_scale(v: Vec2, s: number): Vec2 {
  return { x: v.x * s, y: v.y * s };
}

export function vec2_length(v: Vec2): number {
  return Math.sqrt(v.x * v.x + v.y * v.y);
}
