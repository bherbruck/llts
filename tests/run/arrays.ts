// Expected output: 60

function main(): void {
  const arr: f64[] = [10, 20, 30];
  let sum: f64 = 0;
  for (const x of arr) {
    sum = sum + x;
  }
  print(sum);
}
