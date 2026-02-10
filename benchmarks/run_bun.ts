// Bun runner — polyfills print() and runs the named benchmark.

(globalThis as any).print = console.log;

const benchName = Bun.argv[2] || "fib";
const mod = await import(`./${benchName}`);
mod.main();
