// N-body simulation — tests struct creation/return in hot loops and Math.sqrt.
// Uses functional style (returns new structs) since field mutation isn't supported yet.

interface Vec3 {
  x: number;
  y: number;
  z: number;
}

function vec3_add(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x + b.x, y: a.y + b.y, z: a.z + b.z };
}

function vec3_sub(a: Vec3, b: Vec3): Vec3 {
  return { x: a.x - b.x, y: a.y - b.y, z: a.z - b.z };
}

function vec3_scale(v: Vec3, s: number): Vec3 {
  return { x: v.x * s, y: v.y * s, z: v.z * s };
}

function vec3_dot(a: Vec3, b: Vec3): number {
  return a.x * b.x + a.y * b.y + a.z * b.z;
}

function vec3_length(v: Vec3): number {
  return Math.sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
}

// Simulate gravitational interaction between two points.
// Returns the velocity delta for point a due to point b with given mass.
function gravity(a: Vec3, b: Vec3, mass_b: number, dt: number): Vec3 {
  const d: Vec3 = vec3_sub(b, a);
  const dist: number = vec3_length(d);
  const mag: number = dt / (dist * dist * dist);
  return vec3_scale(d, mass_b * mag);
}

export function main(): void {
  const steps: number = 20000000;
  const dt: number = 0.001;

  // Two-body orbit
  let p1: Vec3 = { x: 1.0, y: 0.0, z: 0.0 };
  let v1: Vec3 = { x: 0.0, y: 0.5, z: 0.0 };
  let p2: Vec3 = { x: -1.0, y: 0.0, z: 0.0 };
  let v2: Vec3 = { x: 0.0, y: -0.5, z: 0.0 };

  const m1: number = 1.0;
  const m2: number = 1.0;

  let i: number = 0;
  while (i < steps) {
    const dv1: Vec3 = gravity(p1, p2, m2, dt);
    const dv2: Vec3 = gravity(p2, p1, m1, dt);

    v1 = vec3_add(v1, dv1);
    v2 = vec3_add(v2, dv2);

    p1 = vec3_add(p1, vec3_scale(v1, dt));
    p2 = vec3_add(p2, vec3_scale(v2, dt));

    i = i + 1;
  }

  // Print total energy as checksum
  const ke: number = 0.5 * m1 * vec3_dot(v1, v1) + 0.5 * m2 * vec3_dot(v2, v2);
  const d: Vec3 = vec3_sub(p1, p2);
  const dist: number = vec3_length(d);
  const pe: number = -1.0 * m1 * m2 / dist;
  print(ke + pe);
}
