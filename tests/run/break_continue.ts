// Expected output: 3\n12
// Expected output: 12

function main(): void {
  let i: f64 = 0;
  while (i < 10) {
    if (i == 3) {
      break;
    }
    i = i + 1;
  }
  print(i);

  let sum: f64 = 0;
  for (let j: f64 = 0; j < 6; j++) {
    if (j == 3) {
      continue;
    }
    sum = sum + j;
  }
  print(sum);
}
