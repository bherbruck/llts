// Arrow function with expression body
const add = (x: f64, y: f64): f64 => x + y;

// Arrow function with block body
const multiply = (x: f64, y: f64): f64 => {
  return x * y;
};

function main(): void {
  const a: f64 = add(3.0, 4.0);
  const b: f64 = multiply(2.0, 5.0);
  console.log(a);
  console.log(b);
}
