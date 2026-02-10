// Should compile: if/else, while, for loops.

function abs(n: i32): i32 {
  if (n < 0) {
    return -n;
  } else {
    return n;
  }
}

function sum_to(n: i32): i32 {
  let total: i32 = 0;
  let i: i32 = 1;
  while (i <= n) {
    total = total + i;
    i = i + 1;
  }
  return total;
}

function main(): void {
  print(abs(-5));
  print(sum_to(10));
}
