// Expected output: 55

function sum_to(n: f64): f64 {
  let total: f64 = 0;
  let i: f64 = 1;
  while (i <= n) {
    total = total + i;
    i = i + 1;
  }
  return total;
}

function main(): void {
  print(sum_to(10));
}
