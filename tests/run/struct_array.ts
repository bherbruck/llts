// Expected output: 1\n2\n3\n4
type Point = { x: f64; y: f64 };

function main(): void {
  const p1: Point = { x: 1, y: 2 };
  const p2: Point = { x: 3, y: 4 };
  const arr: Point[] = [p1, p2];
  print(arr[0].x);
  print(arr[0].y);
  print(arr[1].x);
  print(arr[1].y);
}
