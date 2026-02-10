// Mandelbrot set — tests nested loops and floating-point arithmetic.

function mandelbrot_iter(cx: number, cy: number, max_iter: number): number {
  let zx: number = 0;
  let zy: number = 0;
  let i: number = 0;
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
    i = i + 1;
  }
  return max_iter;
}

export function main(): void {
  const size: number = 2000;
  const max_iter: number = 1000;
  let total: number = 0;
  let cx: number = 0;
  let cy: number = 0;

  let y: number = 0;
  while (y < size) {
    let x: number = 0;
    while (x < size) {
      cx = (x / size) * 3.5 - 2.5;
      cy = (y / size) * 2.0 - 1.0;
      total = total + mandelbrot_iter(cx, cy, max_iter);
      x = x + 1;
    }
    y = y + 1;
  }

  print(total);
}
