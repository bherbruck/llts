// Expected output: 5

interface Point {
  x: f64;
  y: f64;
}

function makePoint(x: f64, y: f64): Point {
  return { x: x, y: y };
}

function pointSum(p: Point): f64 {
  return p.x + p.y;
}

function main(): void {
  let p: Point = makePoint(2, 3);
  print(pointSum(p));
}
