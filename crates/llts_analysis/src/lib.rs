pub mod borrow;
pub mod monomorph;
pub mod ownership;
pub mod types;
pub mod validate;

use oxc_ast::ast::*;
use oxc_span::Span;

use borrow::{BorrowChecker, BorrowError};
use monomorph::Monomorphizer;
use ownership::{FunctionOwnership, OwnershipAnalyzer, OwnershipError};
use types::{LltsType, TypeRegistry, TypeResolver};
use validate::{ValidationError, Validator};

// ---------------------------------------------------------------------------
// Analysis result
// ---------------------------------------------------------------------------

/// The complete result of analyzing a program.
#[derive(Debug)]
pub struct AnalysisResult {
    /// The type registry containing all resolved types.
    pub registry: TypeRegistry,
    /// Monomorphizer with all generic instantiations.
    pub monomorphizer: Monomorphizer,
    /// Ownership information for each analyzed function.
    pub function_ownership: Vec<FunctionOwnership>,
    /// All errors (validation + ownership + borrow).
    pub errors: Vec<AnalysisError>,
}

impl AnalysisResult {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Unified error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum AnalysisError {
    Validation(ValidationError),
    Ownership(OwnershipError),
    Borrow(BorrowError),
}

impl std::fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisError::Validation(e) => write!(f, "validation: {e}"),
            AnalysisError::Ownership(e) => write!(f, "ownership: {e}"),
            AnalysisError::Borrow(e) => write!(f, "borrow: {e}"),
        }
    }
}

impl AnalysisError {
    pub fn span(&self) -> Span {
        match self {
            AnalysisError::Validation(e) => e.span,
            AnalysisError::Ownership(e) => e.span,
            AnalysisError::Borrow(e) => e.span,
        }
    }
}

// ---------------------------------------------------------------------------
// Main analysis entry point
// ---------------------------------------------------------------------------

/// Analyze a parsed program AST.
///
/// This runs all analysis passes:
/// 1. Subset validation (reject unsupported patterns)
/// 2. Type resolution (AST type annotations -> LltsType IR)
/// 3. Ownership analysis (stack vs heap, move tracking)
/// 4. Borrow checking (Readonly enforcement, use-after-move)
/// 5. Generic monomorphization tracking
pub fn analyze(program: &Program<'_>) -> AnalysisResult {
    let mut errors = Vec::new();

    // -- Pass 1: Validation --
    let validator = Validator::new();
    let validation_errors = validator.validate_program(program);
    errors.extend(validation_errors.into_iter().map(AnalysisError::Validation));

    // -- Pass 2: Type resolution --
    let mut registry = TypeRegistry::new();
    let mut monomorphizer = Monomorphizer::new();

    {
        let mut resolver = TypeResolver::new(&mut registry);

        // First pass: register all top-level type declarations
        for stmt in &program.body {
            match stmt {
                Statement::TSInterfaceDeclaration(iface) => {
                    resolver.resolve_interface(iface);
                }
                Statement::TSTypeAliasDeclaration(alias) => {
                    resolver.resolve_type_alias(alias);
                }
                Statement::TSEnumDeclaration(enum_decl) => {
                    resolve_enum(&mut resolver, enum_decl);
                }
                Statement::ClassDeclaration(class) => {
                    resolve_class(&mut resolver, class);
                }
                // Register generic functions
                Statement::FunctionDeclaration(func) => {
                    if let Some(type_params) = &func.type_parameters {
                        if !type_params.params.is_empty() {
                            register_generic_function(&mut resolver, &mut monomorphizer, func);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // -- Pass 3 & 4: Ownership + Borrow analysis --
    let mut function_ownership = Vec::new();

    let resolve_type_fn = |ts_type: &TSType<'_>| -> LltsType {
        resolve_type_simple(ts_type, &registry)
    };

    for stmt in &program.body {
        match stmt {
            Statement::FunctionDeclaration(func) => {
                // Ownership analysis
                let mut ownership_analyzer = OwnershipAnalyzer::new();
                let func_ownership =
                    ownership_analyzer.analyze_function(func, &resolve_type_fn);
                let ownership_errors = ownership_analyzer.finish();
                errors.extend(ownership_errors.into_iter().map(AnalysisError::Ownership));
                function_ownership.push(func_ownership);

                // Borrow checking
                let mut borrow_checker = BorrowChecker::new();
                borrow_checker.check_function(func, &resolve_type_fn);
                let borrow_errors = borrow_checker.finish();
                errors.extend(borrow_errors.into_iter().map(AnalysisError::Borrow));
            }
            Statement::ClassDeclaration(class) => {
                for element in &class.body.body {
                    if let ClassElement::MethodDefinition(method) = element {
                        let mut ownership_analyzer = OwnershipAnalyzer::new();
                        let func_ownership = ownership_analyzer
                            .analyze_function(&method.value, &resolve_type_fn);
                        let ownership_errors = ownership_analyzer.finish();
                        errors.extend(
                            ownership_errors.into_iter().map(AnalysisError::Ownership),
                        );
                        function_ownership.push(func_ownership);

                        let mut borrow_checker = BorrowChecker::new();
                        borrow_checker.check_function(&method.value, &resolve_type_fn);
                        let borrow_errors = borrow_checker.finish();
                        errors.extend(borrow_errors.into_iter().map(AnalysisError::Borrow));
                    }
                }
            }
            _ => {}
        }
    }

    AnalysisResult {
        registry,
        monomorphizer,
        function_ownership,
        errors,
    }
}

// ---------------------------------------------------------------------------
// Internal helpers for type registration
// ---------------------------------------------------------------------------

fn resolve_enum(resolver: &mut TypeResolver<'_>, enum_decl: &TSEnumDeclaration<'_>) {
    let name = enum_decl.id.name.to_string();
    let mut variants = Vec::new();

    for (i, member) in enum_decl.body.members.iter().enumerate() {
        let variant_name = enum_member_name(&member.id);
        let value = if let Some(init) = &member.initializer {
            match init {
                Expression::NumericLiteral(num) => {
                    types::EnumValue::Numeric(num.value as i64)
                }
                Expression::StringLiteral(s) => {
                    types::EnumValue::String(s.value.to_string())
                }
                _ => types::EnumValue::Numeric(i as i64),
            }
        } else {
            types::EnumValue::Numeric(i as i64)
        };

        variants.push(types::EnumVariant {
            name: variant_name,
            tag: i as i32,
            value,
        });
    }

    let ty = LltsType::Enum(types::EnumType {
        name: name.clone(),
        variants,
        is_const: enum_decl.r#const,
    });

    resolver.registry.register(name, ty);
}

fn resolve_class(resolver: &mut TypeResolver<'_>, class: &Class<'_>) {
    let name = class
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_else(|| "<anonymous_class>".to_string());

    let type_params: Vec<String> = class
        .type_parameters
        .as_ref()
        .map(|tp| {
            tp.params
                .iter()
                .map(|p| p.name.name.to_string())
                .collect()
        })
        .unwrap_or_default();

    let mut fields = Vec::new();
    for element in &class.body.body {
        if let ClassElement::PropertyDefinition(prop) = element {
            if !prop.computed {
                let field_name = property_key_name(&prop.key);
                let ty = prop
                    .type_annotation
                    .as_ref()
                    .map(|ann| resolver.resolve_ts_type(&ann.type_annotation))
                    .unwrap_or(LltsType::Unknown);
                fields.push(types::StructField {
                    name: field_name,
                    ty,
                    readonly: prop.readonly,
                    optional: prop.r#override, // Classes don't have `optional` on props directly
                });
            }
        }
    }

    let ty = LltsType::Struct(types::StructType {
        name: name.clone(),
        fields,
        type_params,
    });

    resolver.registry.register(name, ty);
}

fn register_generic_function(
    resolver: &mut TypeResolver<'_>,
    monomorphizer: &mut Monomorphizer,
    func: &Function<'_>,
) {
    let name = func
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_else(|| "<anonymous>".to_string());

    let type_params: Vec<String> = func
        .type_parameters
        .as_ref()
        .map(|tp| {
            tp.params
                .iter()
                .map(|p| p.name.name.to_string())
                .collect()
        })
        .unwrap_or_default();

    let params: Vec<types::FunctionParam> = func
        .params
        .items
        .iter()
        .map(|param| {
            let param_name = binding_pattern_name(&param.pattern);
            let ty = param
                .type_annotation
                .as_ref()
                .map(|ann| resolver.resolve_ts_type(&ann.type_annotation))
                .unwrap_or(LltsType::Unknown);
            types::FunctionParam {
                name: param_name,
                ty,
            }
        })
        .collect();

    let return_type = func
        .return_type
        .as_ref()
        .map(|ret| resolver.resolve_ts_type(&ret.type_annotation))
        .unwrap_or(LltsType::Void);

    let func_type = types::FunctionType {
        params,
        return_type: Box::new(return_type),
        type_params: type_params.clone(),
    };

    monomorphizer.register_generic_function(name, type_params, func_type);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn binding_pattern_name(pattern: &BindingPattern<'_>) -> String {
    match pattern {
        BindingPattern::BindingIdentifier(id) => id.name.to_string(),
        _ => "_".to_string(),
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> String {
    match key {
        PropertyKey::StaticIdentifier(id) => id.name.to_string(),
        _ => "<computed>".to_string(),
    }
}

fn enum_member_name(name: &TSEnumMemberName<'_>) -> String {
    match name {
        TSEnumMemberName::Identifier(id) => id.name.to_string(),
        TSEnumMemberName::String(s) => s.value.to_string(),
        TSEnumMemberName::ComputedString(s) => s.value.to_string(),
        TSEnumMemberName::ComputedTemplateString(_) => "<computed>".to_string(),
    }
}

/// Resolve a TSType to an LltsType using only an immutable registry reference.
/// Handles primitives and named type lookups without mutating the registry.
fn resolve_type_simple(ts_type: &TSType<'_>, registry: &TypeRegistry) -> LltsType {
    match ts_type {
        TSType::TSNumberKeyword(_) => LltsType::Number,
        TSType::TSBooleanKeyword(_) => LltsType::Boolean,
        TSType::TSStringKeyword(_) => LltsType::String,
        TSType::TSVoidKeyword(_) => LltsType::Void,
        TSType::TSNeverKeyword(_) => LltsType::Never,
        TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => LltsType::Void,
        TSType::TSTypeReference(type_ref) => {
            let name = match &type_ref.type_name {
                TSTypeName::IdentifierReference(id) => id.name.as_str(),
                TSTypeName::QualifiedName(_) | TSTypeName::ThisExpression(_) => return LltsType::Unknown,
            };
            match name {
                "i8" => LltsType::I8,
                "i16" => LltsType::I16,
                "i32" => LltsType::I32,
                "i64" => LltsType::I64,
                "u8" => LltsType::U8,
                "u16" => LltsType::U16,
                "u32" => LltsType::U32,
                "u64" => LltsType::U64,
                "f32" => LltsType::F32,
                "f64" => LltsType::F64,
                other => registry.get(other).cloned().unwrap_or(LltsType::Unknown),
            }
        }
        TSType::TSArrayType(arr) => {
            let elem = resolve_type_simple(&arr.element_type, registry);
            LltsType::Array(Box::new(elem))
        }
        _ => LltsType::Unknown,
    }
}
