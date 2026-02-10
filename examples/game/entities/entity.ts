// Entity — a game object with position and velocity.

import { Vec2 } from "../math/vec2";

export interface Entity {
  x: number;
  y: number;
  vx: number;
  vy: number;
}

export function create_entity(x: number, y: number, vx: number, vy: number): Entity {
  return { x: x, y: y, vx: vx, vy: vy };
}

export function update_entity(e: Entity, dt: number): Entity {
  return {
    x: e.x + e.vx * dt,
    y: e.y + e.vy * dt,
    vx: e.vx,
    vy: e.vy,
  };
}

export function entity_position(e: Entity): Vec2 {
  return { x: e.x, y: e.y };
}
