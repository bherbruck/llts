// Expected output: true\nfalse\ntrue\nfalse\ntrue\nfalse\nfalse\ntrue

function main(): void {
  // Bool literals
  print(true);
  print(false);

  // Bool from comparison
  if (5 > 3) {
    print("true");
  } else {
    print("false");
  }
  if (3 > 5) {
    print("true");
  } else {
    print("false");
  }

  // Bool variables
  let a: boolean = true;
  let b: boolean = false;
  print(a);
  print(b);

  // Logical operators on bool variables
  let c: boolean = a && b;
  print(c);

  let d: boolean = a || b;
  print(d);
}
