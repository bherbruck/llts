// Expected output: 3.14\n42\nhello
function wrap<T = f64>(x: T): T {
  return x;
}

function pair<A = i32, B = string>(a: A, b: B): A {
  return a;
}

function main(): void {
  // No type args — uses default T = f64
  let a: f64 = wrap(3.14);
  print(a);

  // Partial type args — A = i32, B defaults to string
  let b: i32 = pair<i32>(42 as i32, "hello");
  print(b);

  // Explicit override of defaults
  let c: string = wrap<string>("hello");
  print(c);
}
