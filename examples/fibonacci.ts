// Fibonacci — demonstrates recursion and `as` type casting.

function fibonacci(n: f64): f64 {
  if (n <= 1) {
    return n;
  }
  return fibonacci(n - 1) + fibonacci(n - 2);
}

function main(): void {
  const result: f64 = fibonacci(10);
  print(result);
}
