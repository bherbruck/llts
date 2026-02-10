# Map / Set (v2)

Deferred to v2 stdlib. v1 covers most use cases with arrays and structs.

## v2 Path

Stdlib types written in the compilable TS subset, backed by a hash map implementation.

- `Map<K, V>` — hash map, requires `K` to be hashable (primitives, strings, or types implementing a hash interface)
- `Set<T>` — hash set, same hashability requirement
- `WeakMap<K, V>` / `WeakSet<T>` — may require special compiler support for weak references
