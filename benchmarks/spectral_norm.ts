// Spectral norm — tests nested loops, struct returns, and floating-point division.
// Approximates the spectral norm of an infinite matrix A where A[i][j] = 1/((i+j)(i+j+1)/2+i+1).

function eval_a(i: i32, j: i32): number {
  let fi: number = i as number;
  let fj: number = j as number;
  return 1.0 / ((fi + fj) * (fi + fj + 1.0) / 2.0 + fi + 1.0);
}

function eval_a_times_u(n: i32, u_start: number, round: i32): number {
  let sum: number = 0.0;
  let j: i32 = 0 as i32;
  while (j < n) {
    sum = sum + eval_a(round, j) * u_start;
    j = j + (1 as i32);
  }
  return sum;
}

function approximate(n: i32): number {
  let u: number = 1.0;
  let v: number = 0.0;

  let iter: i32 = 0 as i32;
  while (iter < (10 as i32)) {
    let au: number = 0.0;
    let i: i32 = 0 as i32;
    while (i < n) {
      let j: i32 = 0 as i32;
      let row_sum: number = 0.0;
      while (j < n) {
        row_sum = row_sum + eval_a(i, j);
        j = j + (1 as i32);
      }
      au = au + row_sum * u;
      i = i + (1 as i32);
    }

    let atau: number = 0.0;
    i = 0 as i32;
    while (i < n) {
      let j: i32 = 0 as i32;
      let row_sum: number = 0.0;
      while (j < n) {
        row_sum = row_sum + eval_a(j, i);
        j = j + (1 as i32);
      }
      atau = atau + row_sum * au;
      i = i + (1 as i32);
    }

    v = atau;
    u = v / (n as number);
    iter = iter + (1 as i32);
  }

  return Math.sqrt(v / ((n as number) * u));
}

export function main(): void {
  const result: number = approximate(3000 as i32);
  print(result);
}
