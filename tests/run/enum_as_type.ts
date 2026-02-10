// Expected output: 0\n1\n2

enum Color {
  Red,
  Green,
  Blue,
}

function colorName(c: Color): i32 {
  return c;
}

function main(): void {
  const r: Color = Color.Red;
  console.log(r);
  console.log(colorName(Color.Green));
  console.log(colorName(Color.Blue));
}
