# Compilation Pipeline

```
source.ts → oxc_parser → oxc_semantic → analysis → codegen → LLVM → native binary
```

## Stage 1: Parse

```rust
use oxc_allocator::Allocator;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;

let allocator = Allocator::default();
let source_type = SourceType::from_path("main.ts").unwrap();
let ParserReturn { program, errors, panicked, .. } =
    Parser::new(&allocator, &source_text, source_type).parse();
```

`program` is the full typed AST. Every type annotation, generic parameter, interface, and type alias is preserved as AST nodes.

## Stage 2: Semantic Analysis

```rust
use oxc_semantic::SemanticBuilder;

let semantic_ret = SemanticBuilder::new()
    .with_check_syntax_error(true)
    .build(&program);

let semantic = semantic_ret.semantic;
// semantic.scopes()  — scope tree
// semantic.symbols() — symbol table
// semantic.nodes()   — AST node relationships
```

Gives us resolved scopes, symbol bindings, and reference resolution across the file. Which variable refers to which declaration, what's in scope where.

## Stage 3: Module Resolution

```rust
use oxc_resolver::{ResolveOptions, Resolver};

let resolver = Resolver::new(ResolveOptions {
    extensions: vec![".ts".into(), ".tsx".into()],
    // reads tsconfig.json paths automatically
    tsconfig: Some(TsconfigOptions { /* ... */ }),
    ..Default::default()
});

// When we encounter: import { Point } from "./geometry"
let resolved = resolver.resolve(&current_dir, "./geometry");
// → /src/geometry.ts
```

Handles the full complexity of TS/JS module resolution so we don't have to.

## Stage 4: Subset Validation (Our Code)

This is where we enforce the "compilable TypeScript" rules. Walk the oxc AST and reject patterns that can't be statically compiled:

**Allowed:**
- Primitive types: `number` (→ f64), `i32`, `u32`, `i64`, `f32`, `f64`, `boolean` (→ i1), `string` (→ `{ ptr, len }`)
- Typed function signatures with explicit return types
- Structs via interfaces/classes with known field types
- Generics (monomorphized at compile time)
- Enums (→ tagged unions)
- Discriminated unions with `switch`/`if` narrowing (TS's existing patterns, no custom `match` syntax)
- Arrays with known element types (→ stack or heap allocated)
- `Readonly<T>` for immutable borrow contracts at function boundaries
- `Weak<T>` for back-references in cyclic type definitions

**Rejected:**
- `any`, `unknown` without narrowing
- Dynamic property access (`obj[computed]` on unknown shapes)
- Untyped function parameters
- `eval`, `with`, `Proxy`, `Reflect`
- Runtime type checking (`typeof` / `instanceof` on arbitrary values)
- Prototype manipulation
- Cyclic type definitions without `Weak<T>`
- `async`/`await` (v1 — requires runtime scheduler)

## Stage 5: Type Resolution (Our Code)

See [type-system.md](../features/type-system.md) for the full type mapping tables.

## Stage 6: Codegen — AST → LLVM IR (Our Code)

Walk the validated, type-resolved AST and emit LLVM IR via Inkwell. Three-pass approach:

**Pass 1: Declarations**
- Register struct types (`context.opaque_struct_type(...)`)
- Store generic definitions for later monomorphization
- Define enum tag layouts

**Pass 2: Function Signatures**
- Declare all function signatures (`module.add_function(...)`)
- Resolve calling conventions
- Handle method receivers

**Pass 3: Function Bodies**
- Emit instructions for each statement/expression
- Handle control flow (if/else → branches, loops → phi nodes)
- Track ownership state (moved, borrowed, mutably borrowed)
- Insert drop calls at scope exit
- Handle defer (→ cleanup blocks)

```rust
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;

let context = Context::create();
let module = context.create_module("main");
let builder = context.create_builder();

// Walking an oxc FunctionDeclaration:
// function add(a: i32, b: i32): i32 { return a + b }
let i32_type = context.i32_type();
let fn_type = i32_type.fn_type(&[i32_type.into(), i32_type.into()], false);
let function = module.add_function("add", fn_type, None);
let entry = context.append_basic_block(function, "entry");
builder.position_at_end(entry);
let a = function.get_nth_param(0).unwrap().into_int_value();
let b = function.get_nth_param(1).unwrap().into_int_value();
let sum = builder.build_int_add(a, b, "sum").unwrap();
builder.build_return(Some(&sum)).unwrap();
```

## Stage 7: Optimization & Emit

```rust
use inkwell::targets::*;
use inkwell::OptimizationLevel;

// Run LLVM optimization passes
let pass_manager = PassManager::create(());
pass_manager.add_instruction_combining_pass();
pass_manager.add_reassociate_pass();
pass_manager.add_gvn_pass();
pass_manager.add_cfg_simplification_pass();
pass_manager.run_on(&module);

// Emit native object file
Target::initialize_native(&InitializationConfig::default()).unwrap();
let target_triple = TargetMachine::get_default_triple();
let target = Target::from_triple(&target_triple).unwrap();
let machine = target.create_target_machine(
    &target_triple,
    "generic", "",
    OptimizationLevel::Aggressive,
    RelocMode::Default,
    CodeModel::Default,
).unwrap();

machine.write_to_file(&module, FileType::Object, Path::new("output.o")).unwrap();
// Then link with system linker (cc/ld) to produce final binary
```
