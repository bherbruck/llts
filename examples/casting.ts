// Casting — demonstrates `as` for type coercion.

function main(): void {
  const x: i32 = 42 as i32;
  const y: f64 = x as f64;
  print(y);
}
