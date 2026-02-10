// Ackermann function — tests deep recursion with integer-style arithmetic.

function ackermann(m: i32, n: i32): i32 {
  if (m == (0 as i32)) {
    return n + (1 as i32);
  }
  if (n == (0 as i32)) {
    return ackermann(m - (1 as i32), 1 as i32);
  }
  return ackermann(m - (1 as i32), ackermann(m, n - (1 as i32)));
}

export function main(): void {
  const result: i32 = ackermann(3 as i32, 12 as i32);
  print(result);
}
