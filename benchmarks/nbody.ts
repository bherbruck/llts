// N-body simulation — full 5-body solar system using field mutation.
// Based on the classic Computer Language Benchmarks Game n-body problem.
// Advance logic is inline since structs are pass-by-value.

interface Body {
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  mass: number;
}

function energy(
  b0x: number, b0y: number, b0z: number, b0vx: number, b0vy: number, b0vz: number, b0m: number,
  b1x: number, b1y: number, b1z: number, b1vx: number, b1vy: number, b1vz: number, b1m: number,
  b2x: number, b2y: number, b2z: number, b2vx: number, b2vy: number, b2vz: number, b2m: number,
  b3x: number, b3y: number, b3z: number, b3vx: number, b3vy: number, b3vz: number, b3m: number,
  b4x: number, b4y: number, b4z: number, b4vx: number, b4vy: number, b4vz: number, b4m: number
): number {
  let e: number = 0.0;

  // Kinetic energy
  e += 0.5 * b0m * (b0vx * b0vx + b0vy * b0vy + b0vz * b0vz);
  e += 0.5 * b1m * (b1vx * b1vx + b1vy * b1vy + b1vz * b1vz);
  e += 0.5 * b2m * (b2vx * b2vx + b2vy * b2vy + b2vz * b2vz);
  e += 0.5 * b3m * (b3vx * b3vx + b3vy * b3vy + b3vz * b3vz);
  e += 0.5 * b4m * (b4vx * b4vx + b4vy * b4vy + b4vz * b4vz);

  // Potential energy for each pair
  let dx: number = b0x - b1x;
  let dy: number = b0y - b1y;
  let dz: number = b0z - b1z;
  e -= b0m * b1m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b0x - b2x; dy = b0y - b2y; dz = b0z - b2z;
  e -= b0m * b2m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b0x - b3x; dy = b0y - b3y; dz = b0z - b3z;
  e -= b0m * b3m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b0x - b4x; dy = b0y - b4y; dz = b0z - b4z;
  e -= b0m * b4m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b1x - b2x; dy = b1y - b2y; dz = b1z - b2z;
  e -= b1m * b2m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b1x - b3x; dy = b1y - b3y; dz = b1z - b3z;
  e -= b1m * b3m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b1x - b4x; dy = b1y - b4y; dz = b1z - b4z;
  e -= b1m * b4m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b2x - b3x; dy = b2y - b3y; dz = b2z - b3z;
  e -= b2m * b3m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b2x - b4x; dy = b2y - b4y; dz = b2z - b4z;
  e -= b2m * b4m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  dx = b3x - b4x; dy = b3y - b4y; dz = b3z - b4z;
  e -= b3m * b4m / Math.sqrt(dx * dx + dy * dy + dz * dz);

  return e;
}

export function main(): void {
  const steps: number = 50000000;
  const dt: number = 0.01;
  const SOLAR_MASS: number = 39.47841760435743;
  const DAYS_PER_YEAR: number = 365.24;

  // Sun
  let sun: Body = {
    x: 0.0, y: 0.0, z: 0.0,
    vx: 0.0, vy: 0.0, vz: 0.0,
    mass: SOLAR_MASS,
  };

  // Jupiter
  let jupiter: Body = {
    x: 4.84143144246472090,
    y: -1.16032004402742839,
    z: -0.103622044471123109,
    vx: 0.00166007664274403694 * DAYS_PER_YEAR,
    vy: 0.00769901118419740425 * DAYS_PER_YEAR,
    vz: -0.0000690460016972063023 * DAYS_PER_YEAR,
    mass: 0.000954791938424326609 * SOLAR_MASS,
  };

  // Saturn
  let saturn: Body = {
    x: 8.34336671824457987,
    y: 4.12479856412430479,
    z: -0.403523417114321381,
    vx: -0.00276742510726862411 * DAYS_PER_YEAR,
    vy: 0.00499852801234917238 * DAYS_PER_YEAR,
    vz: 0.0000230417297573763929 * DAYS_PER_YEAR,
    mass: 0.000285885980666130812 * SOLAR_MASS,
  };

  // Uranus
  let uranus: Body = {
    x: 12.8943695621391310,
    y: -15.1111514016986312,
    z: -0.223307578892655734,
    vx: 0.00296460137564761618 * DAYS_PER_YEAR,
    vy: 0.00237847173959480950 * DAYS_PER_YEAR,
    vz: -0.0000296589568540237556 * DAYS_PER_YEAR,
    mass: 0.0000436624404335156298 * SOLAR_MASS,
  };

  // Neptune
  let neptune: Body = {
    x: 15.3796971148509165,
    y: -25.9193146099879641,
    z: 0.179258772950371181,
    vx: 0.00268067772490389322 * DAYS_PER_YEAR,
    vy: 0.00162824170038242295 * DAYS_PER_YEAR,
    vz: -0.0000951592254519715870 * DAYS_PER_YEAR,
    mass: 0.0000515138902046611451 * SOLAR_MASS,
  };

  // Offset momentum of sun
  let px: number = 0.0;
  let py: number = 0.0;
  let pz: number = 0.0;
  px += jupiter.vx * jupiter.mass;
  py += jupiter.vy * jupiter.mass;
  pz += jupiter.vz * jupiter.mass;
  px += saturn.vx * saturn.mass;
  py += saturn.vy * saturn.mass;
  pz += saturn.vz * saturn.mass;
  px += uranus.vx * uranus.mass;
  py += uranus.vy * uranus.mass;
  pz += uranus.vz * uranus.mass;
  px += neptune.vx * neptune.mass;
  py += neptune.vy * neptune.mass;
  pz += neptune.vz * neptune.mass;
  sun.vx = -1.0 * px / SOLAR_MASS;
  sun.vy = -1.0 * py / SOLAR_MASS;
  sun.vz = -1.0 * pz / SOLAR_MASS;

  print(energy(
    sun.x, sun.y, sun.z, sun.vx, sun.vy, sun.vz, sun.mass,
    jupiter.x, jupiter.y, jupiter.z, jupiter.vx, jupiter.vy, jupiter.vz, jupiter.mass,
    saturn.x, saturn.y, saturn.z, saturn.vx, saturn.vy, saturn.vz, saturn.mass,
    uranus.x, uranus.y, uranus.z, uranus.vx, uranus.vy, uranus.vz, uranus.mass,
    neptune.x, neptune.y, neptune.z, neptune.vx, neptune.vy, neptune.vz, neptune.mass
  ));

  let i: number = 0;
  while (i < steps) {
    // Advance: compute all 10 pair interactions, update velocities via field mutation
    let dx: number = 0.0;
    let dy: number = 0.0;
    let dz: number = 0.0;
    let dist: number = 0.0;
    let mag: number = 0.0;

    // sun-jupiter
    dx = sun.x - jupiter.x; dy = sun.y - jupiter.y; dz = sun.z - jupiter.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    sun.vx -= dx * jupiter.mass * mag;
    sun.vy -= dy * jupiter.mass * mag;
    sun.vz -= dz * jupiter.mass * mag;
    jupiter.vx += dx * sun.mass * mag;
    jupiter.vy += dy * sun.mass * mag;
    jupiter.vz += dz * sun.mass * mag;

    // sun-saturn
    dx = sun.x - saturn.x; dy = sun.y - saturn.y; dz = sun.z - saturn.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    sun.vx -= dx * saturn.mass * mag;
    sun.vy -= dy * saturn.mass * mag;
    sun.vz -= dz * saturn.mass * mag;
    saturn.vx += dx * sun.mass * mag;
    saturn.vy += dy * sun.mass * mag;
    saturn.vz += dz * sun.mass * mag;

    // sun-uranus
    dx = sun.x - uranus.x; dy = sun.y - uranus.y; dz = sun.z - uranus.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    sun.vx -= dx * uranus.mass * mag;
    sun.vy -= dy * uranus.mass * mag;
    sun.vz -= dz * uranus.mass * mag;
    uranus.vx += dx * sun.mass * mag;
    uranus.vy += dy * sun.mass * mag;
    uranus.vz += dz * sun.mass * mag;

    // sun-neptune
    dx = sun.x - neptune.x; dy = sun.y - neptune.y; dz = sun.z - neptune.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    sun.vx -= dx * neptune.mass * mag;
    sun.vy -= dy * neptune.mass * mag;
    sun.vz -= dz * neptune.mass * mag;
    neptune.vx += dx * sun.mass * mag;
    neptune.vy += dy * sun.mass * mag;
    neptune.vz += dz * sun.mass * mag;

    // jupiter-saturn
    dx = jupiter.x - saturn.x; dy = jupiter.y - saturn.y; dz = jupiter.z - saturn.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    jupiter.vx -= dx * saturn.mass * mag;
    jupiter.vy -= dy * saturn.mass * mag;
    jupiter.vz -= dz * saturn.mass * mag;
    saturn.vx += dx * jupiter.mass * mag;
    saturn.vy += dy * jupiter.mass * mag;
    saturn.vz += dz * jupiter.mass * mag;

    // jupiter-uranus
    dx = jupiter.x - uranus.x; dy = jupiter.y - uranus.y; dz = jupiter.z - uranus.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    jupiter.vx -= dx * uranus.mass * mag;
    jupiter.vy -= dy * uranus.mass * mag;
    jupiter.vz -= dz * uranus.mass * mag;
    uranus.vx += dx * jupiter.mass * mag;
    uranus.vy += dy * jupiter.mass * mag;
    uranus.vz += dz * jupiter.mass * mag;

    // jupiter-neptune
    dx = jupiter.x - neptune.x; dy = jupiter.y - neptune.y; dz = jupiter.z - neptune.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    jupiter.vx -= dx * neptune.mass * mag;
    jupiter.vy -= dy * neptune.mass * mag;
    jupiter.vz -= dz * neptune.mass * mag;
    neptune.vx += dx * jupiter.mass * mag;
    neptune.vy += dy * jupiter.mass * mag;
    neptune.vz += dz * jupiter.mass * mag;

    // saturn-uranus
    dx = saturn.x - uranus.x; dy = saturn.y - uranus.y; dz = saturn.z - uranus.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    saturn.vx -= dx * uranus.mass * mag;
    saturn.vy -= dy * uranus.mass * mag;
    saturn.vz -= dz * uranus.mass * mag;
    uranus.vx += dx * saturn.mass * mag;
    uranus.vy += dy * saturn.mass * mag;
    uranus.vz += dz * saturn.mass * mag;

    // saturn-neptune
    dx = saturn.x - neptune.x; dy = saturn.y - neptune.y; dz = saturn.z - neptune.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    saturn.vx -= dx * neptune.mass * mag;
    saturn.vy -= dy * neptune.mass * mag;
    saturn.vz -= dz * neptune.mass * mag;
    neptune.vx += dx * saturn.mass * mag;
    neptune.vy += dy * saturn.mass * mag;
    neptune.vz += dz * saturn.mass * mag;

    // uranus-neptune
    dx = uranus.x - neptune.x; dy = uranus.y - neptune.y; dz = uranus.z - neptune.z;
    dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
    mag = dt / (dist * dist * dist);
    uranus.vx -= dx * neptune.mass * mag;
    uranus.vy -= dy * neptune.mass * mag;
    uranus.vz -= dz * neptune.mass * mag;
    neptune.vx += dx * uranus.mass * mag;
    neptune.vy += dy * uranus.mass * mag;
    neptune.vz += dz * uranus.mass * mag;

    // Update positions
    sun.x += dt * sun.vx;
    sun.y += dt * sun.vy;
    sun.z += dt * sun.vz;
    jupiter.x += dt * jupiter.vx;
    jupiter.y += dt * jupiter.vy;
    jupiter.z += dt * jupiter.vz;
    saturn.x += dt * saturn.vx;
    saturn.y += dt * saturn.vy;
    saturn.z += dt * saturn.vz;
    uranus.x += dt * uranus.vx;
    uranus.y += dt * uranus.vy;
    uranus.z += dt * uranus.vz;
    neptune.x += dt * neptune.vx;
    neptune.y += dt * neptune.vy;
    neptune.z += dt * neptune.vz;

    i = i + 1;
  }

  print(energy(
    sun.x, sun.y, sun.z, sun.vx, sun.vy, sun.vz, sun.mass,
    jupiter.x, jupiter.y, jupiter.z, jupiter.vx, jupiter.vy, jupiter.vz, jupiter.mass,
    saturn.x, saturn.y, saturn.z, saturn.vx, saturn.vy, saturn.vz, saturn.mass,
    uranus.x, uranus.y, uranus.z, uranus.vx, uranus.vy, uranus.vz, uranus.mass,
    neptune.x, neptune.y, neptune.z, neptune.vx, neptune.vy, neptune.vz, neptune.mass
  ));
}
