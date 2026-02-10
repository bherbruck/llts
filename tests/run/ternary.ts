// Expected output: 10\n5
// Expected output: 5

function main(): void {
  let x: f64 = 10;
  let a: f64 = x > 5 ? 10 : 5;
  print(a);
  let b: f64 = x < 5 ? 10 : 5;
  print(b);
}
