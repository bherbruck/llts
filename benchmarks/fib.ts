// Recursive fibonacci — tests function call overhead.

function fib(n: i32): i32 {
  if (n <= (1 as i32)) {
    return n;
  }
  return fib(n - (1 as i32)) + fib(n - (2 as i32));
}

export function main(): void {
  const result: i32 = fib(40 as i32);
  print(result);
}
