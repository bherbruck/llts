// Expected output: 42\n0

function maybe_value(flag: boolean): f64 | null {
  if (flag) {
    return 42.0;
  }
  return null;
}

function main(): void {
  let a: f64 | null = maybe_value(true);
  if (a !== null) {
    print(a);
  }
  let b: f64 | null = null;
  if (b === null) {
    print(0);
  }
}
