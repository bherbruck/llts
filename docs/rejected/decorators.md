# Decorators (Rejected)

Decorators are runtime metaprogramming — they transform classes/methods dynamically. Cannot be compiled statically.

```typescript
// This requires runtime reflection and dynamic class modification:
@Component({ selector: 'app-root' })
class AppComponent { ... }
```

The decorator function receives the class/method as an argument and can modify it arbitrarily at runtime. This is fundamentally incompatible with ahead-of-time compilation where all types and layouts must be known at compile time.
