// Expected output: 42\n3.14\n10\n3\n255\n0\n128

function main(): void {
  let a: i32 = 42 as i32;
  print(a);
  let b: f64 = 3.14;
  print(b);
  let d: f64 = 10.9;
  let e: i32 = d as i32;
  print(e);
  let f: i32 = 3 as i32;
  let g: f64 = f as f64;
  print(g);

  // u8 should print as unsigned (255, not -1)
  let h: u8 = 255 as u8;
  print(h);
  let i: u8 = 0 as u8;
  print(i);
  let j: u8 = 128 as u8;
  print(j);
}
