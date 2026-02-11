type Point = { x: i64; y: i64; };

function main(): void {
  const p: Point = { x: 10, y: 20 };
  console.log(p);
  console.log(`point is: ${p}`);
  console.log(`x=${p.x}, y=${p.y}`);
}
