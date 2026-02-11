// Expected output: 15

function main(): void {
  const arr = [1, 2, 3, 4, 5];
  let sum: f64 = 0;
  for (const num of arr) {
    sum = sum + num;
  }
  print(sum);
}
