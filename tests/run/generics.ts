// Expected output: 42\n3.14\nhello\n42\n10
function identity<T>(x: T): T {
  return x;
}

function first<A, B>(a: A, b: B): A {
  return a;
}

function second<A, B>(a: A, b: B): B {
  return b;
}

function main(): void {
  let a: i32 = identity<i32>(42 as i32);
  print(a);
  let b: f64 = identity<f64>(3.14);
  print(b);
  let c: string = identity<string>("hello");
  print(c);

  // Multiple type params
  let d: i32 = first<i32, f64>(42 as i32, 3.14);
  print(d);
  let e: i32 = second<f64, i32>(3.14, 10 as i32);
  print(e);
}
