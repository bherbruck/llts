// Expected output: yes\nno\nbig
// Expected output: no
// Expected output: big

function main(): void {
  let x: f64 = 10;
  if (x > 5) {
    print("yes");
  } else {
    print("no");
  }

  if (x < 5) {
    print("yes");
  } else {
    print("no");
  }

  if (x < 5) {
    print("small");
  } else if (x < 20) {
    print("big");
  } else {
    print("huge");
  }
}
