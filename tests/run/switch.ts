// Expected output: two\nother
// Expected output: other

function main(): void {
  let x: f64 = 2;
  switch (x) {
    case 1:
      print("one");
      break;
    case 2:
      print("two");
      break;
    case 3:
      print("three");
      break;
    default:
      print("other");
      break;
  }

  let y: f64 = 99;
  switch (y) {
    case 1:
      print("one");
      break;
    default:
      print("other");
      break;
  }
}
