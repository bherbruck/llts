// Expected output: 1\n15
function sum_to(n: i32): f64 {
  let total: f64 = 0;
  let i: f64 = 1;
  while (i <= n) {
    total = total + i;
    i = i + 1;
  }
  return total;
}

function main(): void {
  let x: i32 = 3 as i32;
  let y: f64 = 1.5;
  let cmp: f64 = x > y ? 1 : 0;
  print(cmp);
  print(sum_to(5 as i32));
}
