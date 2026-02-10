// Expected output: 15

function main(): void {
  let arr: f64[] = [1, 2, 3, 4, 5];
  let sum: f64 = 0;
  for (let x of arr) {
    sum = sum + x;
  }
  print(sum);
}
