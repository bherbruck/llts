// Math utilities — clamp, lerp, distance helpers.

import { Vec2, vec2_sub, vec2_length } from "./vec2";

export function clamp(value: number, min: number, max: number): number {
  if (value < min) {
    return min;
  }
  if (value > max) {
    return max;
  }
  return value;
}

export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * t;
}

export function distance(a: Vec2, b: Vec2): number {
  const diff: Vec2 = vec2_sub(a, b);
  return vec2_length(diff);
}
