// Expected output: 45

function main(): void {
  let sum: f64 = 0;
  for (let i: f64 = 0; i < 10; i++) {
    sum = sum + i;
  }
  print(sum);
}
