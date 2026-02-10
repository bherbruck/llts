// Expected output: 10\n20\n30\n0\n1\n2\n5\n6\n7

enum Explicit {
  A = 10,
  B = 20,
  C = 30,
}

enum StringEnum {
  Red = "RED",
  Green = "GREEN",
  Blue = "BLUE",
}

enum Mixed {
  X = 5,
  Y,
  Z,
}

function main(): void {
  // Explicit numeric values
  console.log(Explicit.A);
  console.log(Explicit.B);
  console.log(Explicit.C);

  // String enum gets sequential integer tags
  console.log(StringEnum.Red);
  console.log(StringEnum.Green);
  console.log(StringEnum.Blue);

  // Auto-increment from last explicit value
  console.log(Mixed.X);
  console.log(Mixed.Y);
  console.log(Mixed.Z);
}
