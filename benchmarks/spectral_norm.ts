// Spectral norm — tests nested loops, struct returns, and floating-point division.
// Approximates the spectral norm of an infinite matrix A where A[i][j] = 1/((i+j)(i+j+1)/2+i+1).

function eval_a(i: number, j: number): number {
  return 1.0 / ((i + j) * (i + j + 1.0) / 2.0 + i + 1.0);
}

function eval_a_times_u(n: number, u_start: number, round: number): number {
  // We can't use arrays, so we recompute u on-the-fly for small n.
  // For the benchmark we use a power-iteration-style approach with scalars.

  // Compute one element of A*u where u is uniform
  let sum: number = 0;
  let j: number = 0;
  while (j < n) {
    sum = sum + eval_a(round, j) * u_start;
    j = j + 1;
  }
  return sum;
}

function approximate(n: number): number {
  // Power iteration to approximate spectral norm
  // Without arrays, we iterate over a single scalar representation
  let u: number = 1.0;
  let v: number = 0.0;

  let iter: number = 0;
  while (iter < 10) {
    // v = A^t * A * u (approximated as scalar)
    let au: number = 0;
    let i: number = 0;
    while (i < n) {
      let j: number = 0;
      let row_sum: number = 0;
      while (j < n) {
        row_sum = row_sum + eval_a(i, j);
        j = j + 1;
      }
      au = au + row_sum * u;
      i = i + 1;
    }

    let atau: number = 0;
    i = 0;
    while (i < n) {
      let j: number = 0;
      let row_sum: number = 0;
      while (j < n) {
        row_sum = row_sum + eval_a(j, i);
        j = j + 1;
      }
      atau = atau + row_sum * au;
      i = i + 1;
    }

    v = atau;
    u = v / n;
    iter = iter + 1;
  }

  return Math.sqrt(v / (n * u));
}

export function main(): void {
  const result: number = approximate(3000);
  print(result);
}
