// LLTS Prelude — ambient type declarations for the compilable TypeScript subset.
// These are valid TypeScript that the compiler recognizes as distinct LLVM types.

// Numeric types (ambient declarations — IDEs see these as `number`)
declare type i8 = number;
declare type i16 = number;
declare type i32 = number;
declare type i64 = number;
declare type u8 = number;
declare type u16 = number;
declare type u32 = number;
declare type u64 = number;
declare type f32 = number;
declare type f64 = number;

// Option<T> — represents T | null / T | undefined
declare type Option<T> = T | null;

// Result<T, E> — represents success or failure
declare type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

// Weak<T> — back-reference for breaking cycles in recursive types
declare type Weak<T> = T;

// Built-in print function
declare function print(...args: any[]): void;
