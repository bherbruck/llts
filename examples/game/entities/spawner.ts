// Spawner — creates entities and applies physics.

import { Entity, create_entity, update_entity } from "./entity";
import { clamp } from "../math/utils";

export function spawn_entity_a(): Entity {
  return create_entity(0, 100, 10, 0);
}

export function spawn_entity_b(): Entity {
  return create_entity(50, 100, -5, 5);
}

export function spawn_entity_c(): Entity {
  return create_entity(25, 200, 0, -3);
}

export function apply_gravity(e: Entity, dt: number): Entity {
  const gravity: number = -9.8;
  const new_vy: number = e.vy + gravity * dt;
  const clamped_vy: number = clamp(new_vy, -50, 50);
  return {
    x: e.x,
    y: e.y,
    vx: e.vx,
    vy: clamped_vy,
  };
}

export function tick(e: Entity, dt: number): Entity {
  const with_gravity: Entity = apply_gravity(e, dt);
  return update_entity(with_gravity, dt);
}
