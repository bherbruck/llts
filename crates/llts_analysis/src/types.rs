use std::collections::HashMap;

use oxc_ast::ast::{
    TSFunctionType, TSInterfaceDeclaration, TSSignature, TSType,
    TSTypeAliasDeclaration, TSTypeReference, TSTypeName,
};

// ---------------------------------------------------------------------------
// Type IDs – lightweight handles into the TypeRegistry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

// ---------------------------------------------------------------------------
// Compiler IR type system
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum LltsType {
    // -- Primitives ----------------------------------------------------------
    /// `number` -> f64 (default JS number semantics)
    Number,
    /// Signed integer types: i8, i16, i32, i64
    I8,
    I16,
    I32,
    I64,
    /// Unsigned integer types: u8, u16, u32, u64
    /// Same LLVM integer types as signed, but different arithmetic ops.
    U8,
    U16,
    U32,
    U64,
    /// Floating-point types
    F32,
    F64,
    /// `boolean` -> i1
    Boolean,
    /// `string` -> { ptr, len } fat pointer to UTF-8 data
    String,
    /// `void` -> LLVM void
    Void,
    /// `never` -> unreachable
    Never,

    // -- Compound types ------------------------------------------------------
    /// Named struct from `interface`, `type` object shape, or `class`.
    /// Fields are ordered as declared; names are preserved for codegen debug info.
    Struct(StructType),

    /// `T[]` -> Vec-like { ptr, len, cap } on the heap.
    Array(Box<LltsType>),

    /// `[T1, T2, ...]` -> fixed-size struct.
    Tuple(Vec<LltsType>),

    /// `A | B` -> { tag: i32, payload: union(A, B) } tagged union.
    Union(UnionType),

    /// `enum Foo { A, B }` -> { tag: i32, payload } (numeric or string enum).
    Enum(EnumType),

    /// `T | null` / `T | undefined` -> { tag: i1, value: T }.
    /// Uses null-pointer optimization for pointer types.
    Option(Box<LltsType>),

    /// `Result<T, E>` -> { tag: i32, union(T, E) }.
    Result {
        ok: Box<LltsType>,
        err: Box<LltsType>,
    },

    /// `(args) => ret` -> { fn_ptr, env_ptr } fat pointer.
    Function(FunctionType),

    /// A generic type that has not yet been monomorphized.
    /// Resolved to a concrete type at each call/usage site.
    Generic(GenericType),

    /// `Readonly<T>` wrapper – indicates an immutable borrow contract.
    Readonly(Box<LltsType>),

    /// `Weak<T>` wrapper – back-reference in cyclic types (no ownership).
    Weak(Box<LltsType>),

    /// A type alias that resolves to another type (no new LLVM type).
    Alias {
        name: std::string::String,
        inner: Box<LltsType>,
    },

    /// Reference to a named type by TypeId (for forward references / recursion).
    Ref(TypeId),

    /// Placeholder for unresolved types during analysis.
    Unknown,
}

// ---------------------------------------------------------------------------
// Compound type details
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: std::string::String,
    pub ty: LltsType,
    pub readonly: bool,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructType {
    pub name: std::string::String,
    pub fields: Vec<StructField>,
    pub type_params: Vec<std::string::String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionVariant {
    pub tag: i32,
    pub ty: LltsType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnionType {
    pub name: Option<std::string::String>,
    pub variants: Vec<UnionVariant>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: std::string::String,
    pub tag: i32,
    pub value: EnumValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnumValue {
    /// Auto-assigned numeric value.
    Numeric(i64),
    /// Explicit string value (compile-time only, stored as tag at runtime).
    String(std::string::String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumType {
    pub name: std::string::String,
    pub variants: Vec<EnumVariant>,
    pub is_const: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    pub name: std::string::String,
    pub ty: LltsType,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionType {
    pub params: Vec<FunctionParam>,
    pub return_type: Box<LltsType>,
    pub type_params: Vec<std::string::String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenericType {
    pub name: std::string::String,
    pub type_params: Vec<std::string::String>,
    pub base: Box<LltsType>,
}

// ---------------------------------------------------------------------------
// Type registry – stores all named types, resolves structural equivalence
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct TypeRegistry {
    /// Named type store: name -> (TypeId, LltsType).
    types: HashMap<std::string::String, (TypeId, LltsType)>,
    /// Reverse lookup: TypeId -> name.
    id_to_name: HashMap<TypeId, std::string::String>,
    next_id: u32,
}

impl TypeRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a named type, returning its TypeId.
    pub fn register(&mut self, name: std::string::String, ty: LltsType) -> TypeId {
        if let Some((id, _)) = self.types.get(&name) {
            return *id;
        }
        let id = TypeId(self.next_id);
        self.next_id += 1;
        self.id_to_name.insert(id, name.clone());
        self.types.insert(name, (id, ty));
        id
    }

    /// Update a previously-registered type (e.g. after forward-reference resolution).
    pub fn update(&mut self, name: &str, ty: LltsType) {
        if let Some(entry) = self.types.get_mut(name) {
            entry.1 = ty;
        }
    }

    /// Look up a type by name.
    pub fn get(&self, name: &str) -> Option<&LltsType> {
        self.types.get(name).map(|(_, ty)| ty)
    }

    /// Look up a type by TypeId.
    pub fn get_by_id(&self, id: TypeId) -> Option<&LltsType> {
        let name = self.id_to_name.get(&id)?;
        self.types.get(name).map(|(_, ty)| ty)
    }

    /// Get the TypeId for a named type.
    pub fn id_of(&self, name: &str) -> Option<TypeId> {
        self.types.get(name).map(|(id, _)| *id)
    }

    /// Get the name for a TypeId.
    pub fn name_of(&self, id: TypeId) -> Option<&str> {
        self.id_to_name.get(&id).map(|s| s.as_str())
    }

    /// Check structural equivalence between two types.
    pub fn structurally_equal(&self, a: &LltsType, b: &LltsType) -> bool {
        match (a, b) {
            // Resolve Ref through the registry
            (LltsType::Ref(id_a), LltsType::Ref(id_b)) if id_a == id_b => true,
            (LltsType::Ref(id), other) | (other, LltsType::Ref(id)) => {
                if let Some(resolved) = self.get_by_id(*id) {
                    self.structurally_equal(resolved, other)
                } else {
                    false
                }
            }
            // Resolve aliases
            (LltsType::Alias { inner, .. }, other) | (other, LltsType::Alias { inner, .. }) => {
                self.structurally_equal(inner, other)
            }
            // Struct: same fields in order
            (LltsType::Struct(a), LltsType::Struct(b)) => {
                a.fields.len() == b.fields.len()
                    && a.fields.iter().zip(&b.fields).all(|(fa, fb)| {
                        fa.name == fb.name && self.structurally_equal(&fa.ty, &fb.ty)
                    })
            }
            // Array
            (LltsType::Array(a), LltsType::Array(b)) => self.structurally_equal(a, b),
            // Tuple
            (LltsType::Tuple(a), LltsType::Tuple(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .zip(b.iter())
                        .all(|(ta, tb)| self.structurally_equal(ta, tb))
            }
            // Function
            (LltsType::Function(a), LltsType::Function(b)) => {
                a.params.len() == b.params.len()
                    && a.params
                        .iter()
                        .zip(&b.params)
                        .all(|(pa, pb)| self.structurally_equal(&pa.ty, &pb.ty))
                    && self.structurally_equal(&a.return_type, &b.return_type)
            }
            // Option
            (LltsType::Option(a), LltsType::Option(b)) => self.structurally_equal(a, b),
            // Result
            (
                LltsType::Result { ok: ok_a, err: err_a },
                LltsType::Result { ok: ok_b, err: err_b },
            ) => self.structurally_equal(ok_a, ok_b) && self.structurally_equal(err_a, err_b),
            // Readonly / Weak wrappers
            (LltsType::Readonly(a), LltsType::Readonly(b)) => self.structurally_equal(a, b),
            (LltsType::Weak(a), LltsType::Weak(b)) => self.structurally_equal(a, b),
            // Primitives and exact matches
            _ => a == b,
        }
    }

    /// Iterate all registered types.
    pub fn iter(&self) -> impl Iterator<Item = (&str, TypeId, &LltsType)> {
        self.types
            .iter()
            .map(|(name, (id, ty))| (name.as_str(), *id, ty))
    }
}

// ---------------------------------------------------------------------------
// Type resolution – walks oxc AST type annotations → LltsType
// ---------------------------------------------------------------------------

pub struct TypeResolver<'a> {
    pub registry: &'a mut TypeRegistry,
}

impl<'a> TypeResolver<'a> {
    pub fn new(registry: &'a mut TypeRegistry) -> Self {
        Self { registry }
    }

    /// Resolve a `TSType` AST node to an `LltsType`.
    pub fn resolve_ts_type(&mut self, ts_type: &TSType<'_>) -> LltsType {
        match ts_type {
            // -- Keyword primitives ------------------------------------------
            TSType::TSNumberKeyword(_) => LltsType::Number,
            TSType::TSBooleanKeyword(_) => LltsType::Boolean,
            TSType::TSStringKeyword(_) => LltsType::String,
            TSType::TSVoidKeyword(_) => LltsType::Void,
            TSType::TSNeverKeyword(_) => LltsType::Never,
            TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => {
                // Bare null/undefined treated as void in isolation;
                // in unions they produce Option<T>.
                LltsType::Void
            }

            // -- Rejected keywords -------------------------------------------
            TSType::TSAnyKeyword(_)
            | TSType::TSUnknownKeyword(_)
            | TSType::TSBigIntKeyword(_)
            | TSType::TSSymbolKeyword(_)
            | TSType::TSObjectKeyword(_) => LltsType::Unknown,

            // -- Type references (named types, generics, ambient numerics) ---
            TSType::TSTypeReference(type_ref) => self.resolve_type_reference(type_ref),

            // -- Array -------------------------------------------------------
            TSType::TSArrayType(arr) => {
                let elem = self.resolve_ts_type(&arr.element_type);
                LltsType::Array(Box::new(elem))
            }

            // -- Tuple -------------------------------------------------------
            TSType::TSTupleType(tuple) => {
                let elems: Vec<LltsType> = tuple
                    .element_types
                    .iter()
                    .map(|el| self.resolve_tuple_element(el))
                    .collect();
                LltsType::Tuple(elems)
            }

            // -- Union -------------------------------------------------------
            TSType::TSUnionType(union) => self.resolve_union(&union.types),

            // -- Function type -----------------------------------------------
            TSType::TSFunctionType(func) => self.resolve_function_type(func),

            // -- Object literal type (inline interface) ----------------------
            TSType::TSTypeLiteral(lit) => self.resolve_type_literal(&lit.members),

            // -- Parenthesized -----------------------------------------------
            TSType::TSParenthesizedType(paren) => self.resolve_ts_type(&paren.type_annotation),

            // -- Literal types (string literal, number literal, boolean) ------
            TSType::TSLiteralType(_) => {
                // Literal types in unions are handled by the union resolver.
                // Standalone literals: we infer the base type.
                LltsType::Unknown
            }

            // -- Intersection ------------------------------------------------
            TSType::TSIntersectionType(inter) => {
                // Intersection of object types → merge fields into one struct.
                // For v1, we flatten to a single struct if all members are object shapes.
                self.resolve_intersection(&inter.types)
            }

            // -- Type operator (keyof, readonly, unique) ---------------------
            TSType::TSTypeOperatorType(op) => {
                use oxc_ast::ast::TSTypeOperatorOperator;
                match op.operator {
                    TSTypeOperatorOperator::Readonly => {
                        let inner = self.resolve_ts_type(&op.type_annotation);
                        LltsType::Readonly(Box::new(inner))
                    }
                    _ => LltsType::Unknown,
                }
            }

            // -- Everything else is unsupported in v1 ------------------------
            _ => LltsType::Unknown,
        }
    }

    /// Resolve a named type reference (e.g. `i32`, `Array<T>`, `Point`, `Readonly<T>`).
    fn resolve_type_reference(&mut self, type_ref: &TSTypeReference<'_>) -> LltsType {
        let name = ts_type_name_to_string(&type_ref.type_name);

        // Ambient numeric types from prelude
        match name.as_str() {
            "i8" => return LltsType::I8,
            "i16" => return LltsType::I16,
            "i32" => return LltsType::I32,
            "i64" => return LltsType::I64,
            "u8" => return LltsType::U8,
            "u16" => return LltsType::U16,
            "u32" => return LltsType::U32,
            "u64" => return LltsType::U64,
            "f32" => return LltsType::F32,
            "f64" => return LltsType::F64,
            _ => {}
        }

        // Resolve generic type arguments
        let type_args: Vec<LltsType> = type_ref
            .type_arguments
            .as_ref()
            .map(|args| {
                args.params
                    .iter()
                    .map(|arg| self.resolve_ts_type(arg))
                    .collect()
            })
            .unwrap_or_default();

        // Built-in generic wrappers
        match name.as_str() {
            "Array" => {
                let elem = type_args.into_iter().next().unwrap_or(LltsType::Unknown);
                return LltsType::Array(Box::new(elem));
            }
            "Readonly" => {
                let inner = type_args.into_iter().next().unwrap_or(LltsType::Unknown);
                return LltsType::Readonly(Box::new(inner));
            }
            "Weak" => {
                let inner = type_args.into_iter().next().unwrap_or(LltsType::Unknown);
                return LltsType::Weak(Box::new(inner));
            }
            "Option" => {
                let inner = type_args.into_iter().next().unwrap_or(LltsType::Unknown);
                return LltsType::Option(Box::new(inner));
            }
            "Result" => {
                let mut args = type_args.into_iter();
                let ok = args.next().unwrap_or(LltsType::Unknown);
                let err = args.next().unwrap_or(LltsType::Unknown);
                return LltsType::Result {
                    ok: Box::new(ok),
                    err: Box::new(err),
                };
            }
            _ => {}
        }

        // Look up in registry
        if let Some(id) = self.registry.id_of(&name) {
            return LltsType::Ref(id);
        }

        // Unknown / forward reference – register as placeholder
        let id = self.registry.register(name, LltsType::Unknown);
        LltsType::Ref(id)
    }

    /// Resolve a union type, collapsing `T | null` into `Option<T>`.
    fn resolve_union(&mut self, types: &[TSType<'_>]) -> LltsType {
        let mut resolved: Vec<LltsType> = Vec::new();
        let mut has_null = false;

        for ty in types {
            match ty {
                TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => {
                    has_null = true;
                }
                _ => {
                    resolved.push(self.resolve_ts_type(ty));
                }
            }
        }

        // `T | null` -> Option<T>
        if has_null && resolved.len() == 1 {
            return LltsType::Option(Box::new(resolved.remove(0)));
        }

        // `null` alone
        if resolved.is_empty() {
            return LltsType::Void;
        }

        // Single type (no null)
        if resolved.len() == 1 && !has_null {
            return resolved.remove(0);
        }

        // Multi-type union (with or without null)
        let mut variants: Vec<UnionVariant> = resolved
            .into_iter()
            .enumerate()
            .map(|(i, ty)| UnionVariant {
                tag: i as i32,
                ty,
            })
            .collect();

        // If null is present in a multi-union, add a None variant
        if has_null {
            variants.push(UnionVariant {
                tag: variants.len() as i32,
                ty: LltsType::Void,
            });
        }

        LltsType::Union(UnionType {
            name: None,
            variants,
        })
    }

    /// Resolve a function type annotation.
    fn resolve_function_type(&mut self, func: &TSFunctionType<'_>) -> LltsType {
        let params: Vec<FunctionParam> = func
            .params
            .items
            .iter()
            .map(|param| {
                let name = binding_pattern_name(&param.pattern);
                let ty = param
                    .type_annotation
                    .as_ref()
                    .map(|ann| self.resolve_ts_type(&ann.type_annotation))
                    .unwrap_or(LltsType::Unknown);
                FunctionParam { name, ty }
            })
            .collect();

        let return_type = self.resolve_ts_type(&func.return_type.type_annotation);

        let type_params: Vec<std::string::String> = func
            .type_parameters
            .as_ref()
            .map(|tp| {
                tp.params
                    .iter()
                    .map(|p| p.name.name.to_string())
                    .collect()
            })
            .unwrap_or_default();

        LltsType::Function(FunctionType {
            params,
            return_type: Box::new(return_type),
            type_params,
        })
    }

    /// Resolve an inline object literal type (e.g. `{ x: f64; y: f64 }`).
    fn resolve_type_literal(&mut self, members: &[TSSignature<'_>]) -> LltsType {
        let fields = self.resolve_signatures(members);
        LltsType::Struct(StructType {
            name: std::string::String::new(),
            fields,
            type_params: Vec::new(),
        })
    }

    /// Resolve an intersection type to a merged struct.
    fn resolve_intersection(&mut self, types: &[TSType<'_>]) -> LltsType {
        let mut all_fields = Vec::new();
        for ty in types {
            match self.resolve_ts_type(ty) {
                LltsType::Struct(s) => all_fields.extend(s.fields),
                _ => return LltsType::Unknown,
            }
        }
        LltsType::Struct(StructType {
            name: std::string::String::new(),
            fields: all_fields,
            type_params: Vec::new(),
        })
    }

    /// Resolve a `TSInterfaceDeclaration` and register it.
    pub fn resolve_interface(&mut self, decl: &TSInterfaceDeclaration<'_>) -> TypeId {
        let name = decl.id.name.to_string();
        let type_params: Vec<std::string::String> = decl
            .type_parameters
            .as_ref()
            .map(|tp| {
                tp.params
                    .iter()
                    .map(|p| p.name.name.to_string())
                    .collect()
            })
            .unwrap_or_default();

        let fields = self.resolve_signatures(&decl.body.body);

        let ty = LltsType::Struct(StructType {
            name: name.clone(),
            fields,
            type_params,
        });

        self.registry.register(name, ty)
    }

    /// Resolve a `TSTypeAliasDeclaration` and register it.
    pub fn resolve_type_alias(&mut self, decl: &TSTypeAliasDeclaration<'_>) -> TypeId {
        let name = decl.id.name.to_string();
        let inner = self.resolve_ts_type(&decl.type_annotation);

        let ty = match &inner {
            // If the alias points to a struct/union/enum, give it the alias name
            LltsType::Struct(_) | LltsType::Union(_) | LltsType::Enum(_) => inner,
            // Otherwise wrap in Alias
            _ => LltsType::Alias {
                name: name.clone(),
                inner: Box::new(inner),
            },
        };

        self.registry.register(name, ty)
    }

    /// Resolve TSSignature members into StructFields.
    fn resolve_signatures(&mut self, members: &[TSSignature<'_>]) -> Vec<StructField> {
        let mut fields = Vec::new();
        for member in members {
            if let TSSignature::TSPropertySignature(prop) = member {
                let field_name = property_key_name(&prop.key);
                let ty = prop
                    .type_annotation
                    .as_ref()
                    .map(|ann| self.resolve_ts_type(&ann.type_annotation))
                    .unwrap_or(LltsType::Unknown);
                fields.push(StructField {
                    name: field_name,
                    ty,
                    readonly: prop.readonly,
                    optional: prop.optional,
                });
            }
        }
        fields
    }

    /// Resolve a tuple element (handles both named and unnamed).
    fn resolve_tuple_element(&mut self, elem: &oxc_ast::ast::TSTupleElement<'_>) -> LltsType {
        match elem {
            oxc_ast::ast::TSTupleElement::TSNamedTupleMember(named) => {
                // Named tuple member wraps another TSTupleElement
                self.resolve_tuple_element(&named.element_type)
            }
            oxc_ast::ast::TSTupleElement::TSOptionalType(opt) => {
                let inner = self.resolve_ts_type(&opt.type_annotation);
                LltsType::Option(Box::new(inner))
            }
            oxc_ast::ast::TSTupleElement::TSRestType(rest) => {
                let inner = self.resolve_ts_type(&rest.type_annotation);
                LltsType::Array(Box::new(inner))
            }
            _ => {
                // TSTupleElement inherits from TSType
                self.resolve_ts_type(elem.to_ts_type())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: extract name from a TSTypeName
// ---------------------------------------------------------------------------

pub fn ts_type_name_to_string(name: &TSTypeName<'_>) -> std::string::String {
    match name {
        TSTypeName::IdentifierReference(ident) => ident.name.to_string(),
        TSTypeName::QualifiedName(qual) => {
            let left = ts_type_name_to_string(&qual.left);
            let right = qual.right.name.to_string();
            format!("{left}.{right}")
        }
        TSTypeName::ThisExpression(_) => "this".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Helper: extract name from a BindingPattern
// ---------------------------------------------------------------------------

fn binding_pattern_name(pattern: &oxc_ast::ast::BindingPattern<'_>) -> std::string::String {
    match pattern {
        oxc_ast::ast::BindingPattern::BindingIdentifier(id) => id.name.to_string(),
        _ => "_".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Helper: extract name from a PropertyKey
// ---------------------------------------------------------------------------

fn property_key_name(key: &oxc_ast::ast::PropertyKey<'_>) -> std::string::String {
    match key {
        oxc_ast::ast::PropertyKey::StaticIdentifier(id) => id.name.to_string(),
        _ => "<computed>".to_string(),
    }
}

// ---------------------------------------------------------------------------
// LltsType convenience methods
// ---------------------------------------------------------------------------

impl LltsType {
    /// Whether this type is a primitive (stack-allocated, copy-on-assign).
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            LltsType::Number
                | LltsType::I8
                | LltsType::I16
                | LltsType::I32
                | LltsType::I64
                | LltsType::U8
                | LltsType::U16
                | LltsType::U32
                | LltsType::U64
                | LltsType::F32
                | LltsType::F64
                | LltsType::Boolean
                | LltsType::Void
                | LltsType::Never
        )
    }

    /// Whether this type is Copy (implicitly copied, never moved).
    /// All primitives and simple enums are Copy.
    pub fn is_copy(&self) -> bool {
        self.is_primitive()
    }

    /// Whether this type is a numeric type.
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            LltsType::Number
                | LltsType::I8
                | LltsType::I16
                | LltsType::I32
                | LltsType::I64
                | LltsType::U8
                | LltsType::U16
                | LltsType::U32
                | LltsType::U64
                | LltsType::F32
                | LltsType::F64
        )
    }

    /// Whether this type is an integer type.
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            LltsType::I8
                | LltsType::I16
                | LltsType::I32
                | LltsType::I64
                | LltsType::U8
                | LltsType::U16
                | LltsType::U32
                | LltsType::U64
        )
    }

    /// Whether this type is a signed integer type.
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            LltsType::I8 | LltsType::I16 | LltsType::I32 | LltsType::I64
        )
    }

    /// Whether this type is an unsigned integer type.
    pub fn is_unsigned(&self) -> bool {
        matches!(
            self,
            LltsType::U8 | LltsType::U16 | LltsType::U32 | LltsType::U64
        )
    }

    /// Whether this type is a float type.
    pub fn is_float(&self) -> bool {
        matches!(self, LltsType::Number | LltsType::F32 | LltsType::F64)
    }

    /// Whether this type needs heap allocation.
    pub fn needs_heap(&self) -> bool {
        matches!(
            self,
            LltsType::String
                | LltsType::Array(_)
                | LltsType::Struct(_)
                | LltsType::Union(_)
                | LltsType::Enum(_)
                | LltsType::Function(_)
        )
    }

    /// Size category for determining stack vs heap allocation.
    pub fn is_small_struct(&self) -> bool {
        match self {
            LltsType::Struct(s) => s.fields.len() <= 4 && s.fields.iter().all(|f| f.ty.is_primitive()),
            LltsType::Tuple(elems) => elems.len() <= 4 && elems.iter().all(|t| t.is_primitive()),
            _ => false,
        }
    }

    /// Bit width of integer types (for LLVM codegen).
    pub fn int_bits(&self) -> Option<u32> {
        match self {
            LltsType::Boolean => Some(1),
            LltsType::I8 | LltsType::U8 => Some(8),
            LltsType::I16 | LltsType::U16 => Some(16),
            LltsType::I32 | LltsType::U32 => Some(32),
            LltsType::I64 | LltsType::U64 => Some(64),
            _ => None,
        }
    }

    /// Whether implicit widening from `self` to `target` is allowed.
    pub fn can_widen_to(&self, target: &LltsType) -> bool {
        match (self, target) {
            // Integer widening: i8 -> i16 -> i32 -> i64
            (LltsType::I8, LltsType::I16 | LltsType::I32 | LltsType::I64) => true,
            (LltsType::I16, LltsType::I32 | LltsType::I64) => true,
            (LltsType::I32, LltsType::I64) => true,
            // Unsigned widening: u8 -> u16 -> u32 -> u64
            (LltsType::U8, LltsType::U16 | LltsType::U32 | LltsType::U64) => true,
            (LltsType::U16, LltsType::U32 | LltsType::U64) => true,
            (LltsType::U32, LltsType::U64) => true,
            // Float widening: f32 -> f64
            (LltsType::F32, LltsType::F64 | LltsType::Number) => true,
            // Any integer -> f64/number
            (ty, LltsType::F64 | LltsType::Number) if ty.is_integer() => true,
            _ => false,
        }
    }
}
