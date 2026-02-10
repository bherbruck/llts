// Mandelbrot set — tests nested loops and floating-point arithmetic.

function mandelbrot_iter(cx: number, cy: number, max_iter: i32): i32 {
  let zx: number = 0;
  let zy: number = 0;
  let i: i32 = 0 as i32;
  let zx2: number = 0;
  let zy2: number = 0;
  let new_zx: number = 0;
  while (i < max_iter) {
    zx2 = zx * zx;
    zy2 = zy * zy;
    if (zx2 + zy2 > 4.0) {
      return i;
    }
    new_zx = zx2 - zy2 + cx;
    zy = 2.0 * zx * zy + cy;
    zx = new_zx;
    i = i + (1 as i32);
  }
  return max_iter;
}

export function main(): void {
  const size: i32 = 2000 as i32;
  const max_iter: i32 = 1000 as i32;
  let total: i32 = 0 as i32;
  let cx: number = 0;
  let cy: number = 0;

  let y: i32 = 0 as i32;
  while (y < size) {
    let x: i32 = 0 as i32;
    while (x < size) {
      cx = (x as number / (size as number)) * 3.5 - 2.5;
      cy = (y as number / (size as number)) * 2.0 - 1.0;
      total = total + mandelbrot_iter(cx, cy, max_iter);
      x = x + (1 as i32);
    }
    y = y + (1 as i32);
  }

  print(total);
}
