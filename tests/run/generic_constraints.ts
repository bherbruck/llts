// Expected output: 10\n3.14\n100
type Num = i8 | i16 | i32 | i64 | f32 | f64;

function add<T extends i32 | f64>(a: T, b: T): T {
  return a + b;
}

function double<T extends Num>(x: T): T {
  return x + x;
}

function main(): void {
  let x: i32 = add<i32>(3 as i32, 7 as i32);
  print(x);
  let y: f64 = add<f64>(1.14, 2.0);
  print(y);

  // Constraint via type alias
  let z: i32 = double<i32>(50 as i32);
  print(z);
}
