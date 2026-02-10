# Desugaring

The oxc AST preserves all syntax exactly as written — classes, arrow functions, optional chaining, destructuring, template literals, etc. are all distinct, well-typed AST nodes. We desugar these in `llts_analysis` before codegen. oxc_semantic gives us scope/binding/reference resolution; the desugaring is straightforward pattern matching on the AST.

## Desugaring Table

| Syntax | Desugars to | AST node | Complexity |
|---|---|---|---|
| `class Foo { x: T; method() {} }` | Struct + free functions (`this` → first param) | `Class`, `MethodDefinition`, `PropertyDefinition` | Medium |
| Arrow functions `(x) => x + 1` | Closure with `{ fn_ptr, env_ptr }` | `ArrowFunctionExpression` | Low |
| `obj?.field` optional chaining | `if (obj !== null) obj.field else null` | `ChainExpression` | Low |
| `a ?? b` nullish coalescing | `if (a !== null) a else b` | `LogicalExpression` | Low |
| `const { x, y } = point` destructuring | Individual field accesses | `ObjectPattern`, `ArrayPattern` | Medium |
| `[...a, ...b]` spread | Loop to copy elements | `SpreadElement` | Medium |
| `` `hello ${name}` `` template literals | String concatenation | `TemplateLiteral` | Low |
| `new Foo(args)` | Constructor function call | `NewExpression` | Low |
| `obj.method(args)` | `Foo_method(obj, args)` | `CallExpression` + `StaticMemberExpression` | Low |
| `x **= 2` compound assignment | `x = x ** 2` | `AssignmentExpression` | Low |
| `throw` / `try` / `catch` | `Result<T, E>` return + branching | `ThrowStatement`, `TryStatement` | Medium |
| `finally` | Code emitted in both Ok/Err branches | `TryStatement.finalizer` | Low |

All of these are pattern matches on oxc AST nodes — no special framework needed, just `match` statements in Rust. The oxc_transformer source is a good reference for edge cases in each transform, but we implement our own since theirs targets JS output.

## oxc_transformer as Reference

We don't use oxc_transformer directly (it strips types and targets JS output), but its source code is useful reference:
- Well-organized visitor pattern using the `Traverse` trait
- Individual transform passes for optional chaining, nullish coalescing, arrow functions, class fields, spread, etc.
- Not modular — you set a `target` and all transforms for that target run. Can't cherry-pick.

We implement our own desugaring using the same `Traverse` pattern but targeting our compiler IR instead of JS.
