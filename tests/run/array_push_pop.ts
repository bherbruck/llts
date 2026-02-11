// Expected output: 3\n30\n2

function main(): void {
  let arr: f64[] = [10, 20];
  arr.push(30);
  print(arr.length);
  const last: f64 = arr.pop();
  print(last);
  print(arr.length);
}
