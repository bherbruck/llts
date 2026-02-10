// Expected output: 11\n30
// Expected output: 30

const increment = (x: f64): f64 => x + 1;

const mul = (a: f64, b: f64): f64 => a * b;

function main(): void {
  print(increment(10));
  print(mul(5, 6));
}
