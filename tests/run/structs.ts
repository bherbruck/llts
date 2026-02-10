// Expected output: 3\n4

interface Point {
  x: f64;
  y: f64;
}

function main(): void {
  const p: Point = { x: 3, y: 4 };
  print(p.x);
  print(p.y);
}
