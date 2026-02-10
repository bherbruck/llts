// Simulation — runs a simple physics simulation loop.

import { Entity, entity_position } from "./entities/entity";
import { spawn_entity_a, spawn_entity_b, spawn_entity_c, tick } from "./entities/spawner";
import { Vec2 } from "./math/vec2";
import { distance } from "./math/utils";

export function run_simulation(): void {
  let a: Entity = spawn_entity_a();
  let b: Entity = spawn_entity_b();
  let c: Entity = spawn_entity_c();

  const dt: number = 1.0;
  let step: number = 0;

  while (step < 3) {
    a = tick(a, dt);
    b = tick(b, dt);
    c = tick(c, dt);
    step = step + 1;
  }

  // Print final positions after 3 ticks
  print(a.x);
  print(a.y);
  print(b.x);
  print(b.y);
  print(c.x);
  print(c.y);

  // Print distance between entity a and b
  const pos_a: Vec2 = entity_position(a);
  const pos_b: Vec2 = entity_position(b);
  const d: number = distance(pos_a, pos_b);
  print(d);
}
