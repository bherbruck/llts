// Leibniz series for pi — tests tight loop with floating-point division.

export function main(): void {
  const n: i32 = 100000000 as i32;
  let sum: number = 0.0;
  let sign: i32 = 1 as i32;
  let i: i32 = 0 as i32;

  while (i < n) {
    sum = sum + (sign as number) / (2.0 * (i as number) + 1.0);
    sign = (0 as i32) - sign;
    i = i + (1 as i32);
  }

  print(sum * 4.0);
}
