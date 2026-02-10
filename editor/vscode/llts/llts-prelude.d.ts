// LLTS type prelude — branded numeric types for strict type checking.
//
// f64 is unbranded (= number) since bare numeric literals are f64 by default.
// All other numeric types are branded, requiring explicit `as` casts.

// f64 = number (default numeric type, no cast needed for literals)
type f64 = number;

// Branded integer types — incompatible with each other and with f64.
declare const __i32: unique symbol;
declare const __u32: unique symbol;
declare const __i64: unique symbol;
declare const __u8: unique symbol;

type i32 = number & { readonly [__i32]: true };
type u32 = number & { readonly [__u32]: true };
type i64 = number & { readonly [__i64]: true };
type u8 = number & { readonly [__u8]: true };

// Built-in functions
declare function print(...args: unknown[]): void;
