// Leibniz series for pi — tests tight loop with floating-point division.

export function main(): void {
  const n: number = 100000000;
  let sum: number = 0;
  let sign: number = 1;
  let i: number = 0;

  while (i < n) {
    sum = sum + sign / (2.0 * i + 1.0);
    sign = sign * -1;
    i = i + 1;
  }

  print(sum * 4.0);
}
