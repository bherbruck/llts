// Prime sieve — tests integer arithmetic and tight loops with bitwise ops.

function sieve_count(limit: number): number {
  // We simulate a boolean sieve using arithmetic on a rolling window.
  // Since we don't have arrays with push, we count primes via trial division.
  let count: number = 0;
  let n: number = 2;

  while (n <= limit) {
    let is_prime: number = 1;
    let d: number = 2;
    while (d * d <= n) {
      if (n - Math.floor(n / d) * d == 0) {
        is_prime = 0;
        d = n; // break
      }
      d = d + 1;
    }
    count = count + is_prime;
    n = n + 1;
  }

  return count;
}

export function main(): void {
  const result: number = sieve_count(200000);
  print(result);
}
