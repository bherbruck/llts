# Generators (v2)

Rejected in v1. Same state machine infrastructure as async.

## v2 Path: State Machine Transform

Each `yield` becomes a state in a generated struct. Will share implementation with async.

```typescript
// What the developer writes:
function* range(start: i32, end: i32): Generator<i32> {
  for (let i = start; i < end; i++) {
    yield i;
  }
}

// Compiles to a struct with a next() method:
// State 0: i = start
// State 1: yield i, i++, if i < end goto State 1 else done
```
