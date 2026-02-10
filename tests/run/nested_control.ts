// Expected output: 5

function main(): void {
  let sum: f64 = 0;
  for (let i: f64 = 0; i < 5; i++) {
    let j: f64 = 0;
    while (j < 5) {
      if (i == j) {
        sum = sum + 1;
      }
      j = j + 1;
    }
  }
  print(sum);
}
