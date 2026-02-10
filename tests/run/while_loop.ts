// Expected output: 15

function main(): void {
  let sum: f64 = 0;
  let i: f64 = 1;
  while (i <= 5) {
    sum = sum + i;
    i = i + 1;
  }
  print(sum);
}
