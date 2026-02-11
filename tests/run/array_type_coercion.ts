// Expected output: 6

function main(): void {
  const arr: Array<i64> = [1, 2, 3];
  let sum: i64 = 0 as i64;
  for (const x of arr) {
    sum = sum + x;
  }
  console.log(sum);
}
