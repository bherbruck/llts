# Iterators / for...of

## v1: Arrays Only

`for...of` on arrays compiles to index-based loops (zero overhead):

```typescript
for (const x of arr) { ... }
// → for (let _i = 0; _i < arr.len; _i++) { const x = arr[_i]; ... }
```

No iterator protocol, no allocations, no virtual dispatch. Just a counter and bounds check.

## v2: Iterator Protocol

v2 adds a generic iterator protocol: types implement `next(): Option<T>`, `for...of` calls `next()` in a loop. Enables custom iterable types and lazy sequences.
