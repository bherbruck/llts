// Expected output: 120

function factorial(n: f64): f64 {
  if (n <= 1) {
    return 1;
  }
  return n * factorial(n - 1);
}

function main(): void {
  print(factorial(5));
}
