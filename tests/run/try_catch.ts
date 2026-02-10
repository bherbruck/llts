// Expected output: before\ncaught
// Expected output: caught

function main(): void {
  try {
    print("before");
    throw "oops";
  } catch (e) {
    print("caught");
  }
}
