// Prime sieve — tests integer arithmetic and tight loops.

function sieve_count(limit: i32): i32 {
  let count: i32 = 0 as i32;
  let n: i32 = 2 as i32;

  while (n <= limit) {
    let is_prime: i32 = 1 as i32;
    let d: i32 = 2 as i32;
    while (d * d <= n) {
      if (n % d == (0 as i32)) {
        is_prime = 0 as i32;
        break;
      }
      d = d + (1 as i32);
    }
    count = count + is_prime;
    n = n + (1 as i32);
  }

  return count;
}

export function main(): void {
  const result: i32 = sieve_count(200000 as i32);
  print(result);
}
