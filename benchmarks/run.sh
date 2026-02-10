#!/usr/bin/env bash
set -e

cd "$(dirname "$0")/.."

echo "Building LLTS (release)..."
cargo build --release 2>&1 | tail -1

LLTS=./target/release/llts_cli
BENCHMARKS=(fib mandelbrot leibniz_pi nbody sieve spectral_norm ackermann euler_sum)

for bench in "${BENCHMARKS[@]}"; do
  echo ""
  echo "=== $bench ==="

  $LLTS "benchmarks/$bench.ts" -O2 -o "/tmp/bench_$bench"

  echo ""
  echo "  LLTS (native):"
  time /tmp/bench_$bench

  echo ""
  echo "  Bun:"
  time bun benchmarks/run_bun.ts "$bench"
done
