// Expected output: 1\n-1\ntrue\nfalse

function main(): void {
  const arr: f64[] = [10, 20, 30];
  console.log(arr.indexOf(20));
  console.log(arr.indexOf(99));
  console.log(arr.includes(30));
  console.log(arr.includes(99));
}
