// Should compile: basic function with typed parameters and return.

function add(a: i32, b: i32): i32 {
  return a + b;
}

function main(): void {
  const result: i32 = add(1, 2);
  print(result);
}
