// Euler sum — tests tight floating-point loop computing sum of 1/k^2.
// Converges to pi^2/6. Tests FP division and multiplication throughput.

export function main(): void {
  const n: number = 500000000;
  let sum: number = 0.0;
  let k: number = 1.0;

  while (k <= n) {
    sum = sum + 1.0 / (k * k);
    k = k + 1.0;
  }

  print(sum);
}
