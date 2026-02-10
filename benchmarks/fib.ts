// Recursive fibonacci — tests function call overhead.

function fib(n: number): number {
  if (n <= 1) {
    return n;
  }
  return fib(n - 1) + fib(n - 2);
}

export function main(): void {
  const result: number = fib(40);
  print(result);
}
