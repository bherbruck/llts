// Expected output: 8\n15\nhello
// Expected output: 15
// Expected output: hello

function add(a: f64, b: f64): f64 {
  return a + b;
}

function multiply(x: f64, y: f64): f64 {
  return x * y;
}

function greet(): string {
  return "hello";
}

function main(): void {
  print(add(3, 5));
  print(multiply(3, 5));
  print(greet());
}
