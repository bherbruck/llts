// Ackermann function — tests deep recursion with integer-style arithmetic.

function ackermann(m: number, n: number): number {
  if (m == 0) {
    return n + 1;
  }
  if (n == 0) {
    return ackermann(m - 1, 1);
  }
  return ackermann(m - 1, ackermann(m, n - 1));
}

export function main(): void {
  const result: number = ackermann(3, 12);
  print(result);
}
