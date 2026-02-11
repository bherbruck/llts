// Expected output: 15

const arr = [1, 2, 3, 4, 5];

function main(): void {
  let sum: f64 = 0;
  for (const num of arr) {
    sum = sum + num;
  }
  print(sum);
}
