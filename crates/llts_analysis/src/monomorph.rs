use std::collections::HashMap;

use crate::types::{
    FunctionParam, FunctionType, LltsType, StructField, StructType,
    TypeRegistry, UnionType, UnionVariant,
};

// ---------------------------------------------------------------------------
// Monomorphization instance
// ---------------------------------------------------------------------------

/// A monomorphized instance of a generic function or type.
#[derive(Debug, Clone)]
pub struct MonomorphInstance {
    /// Original generic name (e.g. `identity`).
    pub original_name: String,
    /// Mangled name for codegen (e.g. `identity_i32`).
    pub mangled_name: String,
    /// Concrete type arguments (e.g. `[LltsType::I32]`).
    pub type_args: Vec<LltsType>,
    /// The specialized type (function or struct with generics replaced).
    pub specialized: LltsType,
}

// ---------------------------------------------------------------------------
// Monomorphizer
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Monomorphizer {
    /// All monomorphized instances, keyed by mangled name.
    instances: HashMap<String, MonomorphInstance>,
    /// Generic function definitions: name -> (type_params, LltsType::Function).
    generic_functions: HashMap<String, (Vec<String>, FunctionType)>,
    /// Generic type definitions: name -> (type_params, LltsType).
    generic_types: HashMap<String, (Vec<String>, LltsType)>,
}

impl Monomorphizer {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            generic_functions: HashMap::new(),
            generic_types: HashMap::new(),
        }
    }

    /// Register a generic function definition.
    pub fn register_generic_function(
        &mut self,
        name: String,
        type_params: Vec<String>,
        func_type: FunctionType,
    ) {
        self.generic_functions
            .insert(name, (type_params, func_type));
    }

    /// Register a generic type definition.
    pub fn register_generic_type(
        &mut self,
        name: String,
        type_params: Vec<String>,
        ty: LltsType,
    ) {
        self.generic_types.insert(name, (type_params, ty));
    }

    /// Monomorphize a generic function with concrete type arguments.
    /// Returns the mangled name for codegen.
    pub fn monomorphize_function(
        &mut self,
        name: &str,
        type_args: &[LltsType],
    ) -> Option<String> {
        let (type_params, func_type) = self.generic_functions.get(name)?.clone();
        let mangled = mangle_name(name, type_args);

        // Already monomorphized?
        if self.instances.contains_key(&mangled) {
            return Some(mangled);
        }

        // Build substitution map: type_param_name -> concrete type
        let substitutions: HashMap<&str, &LltsType> = type_params
            .iter()
            .zip(type_args.iter())
            .map(|(param, arg)| (param.as_str(), arg))
            .collect();

        // Substitute in parameter types
        let specialized_params: Vec<FunctionParam> = func_type
            .params
            .iter()
            .map(|p| FunctionParam {
                name: p.name.clone(),
                ty: substitute_type(&p.ty, &substitutions),
            })
            .collect();

        // Substitute in return type
        let specialized_return = substitute_type(&func_type.return_type, &substitutions);

        let specialized = LltsType::Function(FunctionType {
            params: specialized_params,
            return_type: Box::new(specialized_return),
            type_params: Vec::new(), // No longer generic
        });

        self.instances.insert(
            mangled.clone(),
            MonomorphInstance {
                original_name: name.to_string(),
                mangled_name: mangled.clone(),
                type_args: type_args.to_vec(),
                specialized,
            },
        );

        Some(mangled)
    }

    /// Monomorphize a generic type with concrete type arguments.
    pub fn monomorphize_type(
        &mut self,
        name: &str,
        type_args: &[LltsType],
        registry: &mut TypeRegistry,
    ) -> Option<String> {
        let (type_params, base_type) = self.generic_types.get(name)?.clone();
        let mangled = mangle_name(name, type_args);

        if self.instances.contains_key(&mangled) {
            return Some(mangled);
        }

        let substitutions: HashMap<&str, &LltsType> = type_params
            .iter()
            .zip(type_args.iter())
            .map(|(param, arg)| (param.as_str(), arg))
            .collect();

        let specialized = substitute_type(&base_type, &substitutions);

        // Register the specialized type in the registry
        registry.register(mangled.clone(), specialized.clone());

        self.instances.insert(
            mangled.clone(),
            MonomorphInstance {
                original_name: name.to_string(),
                mangled_name: mangled.clone(),
                type_args: type_args.to_vec(),
                specialized,
            },
        );

        Some(mangled)
    }

    /// Get a monomorphized instance by its mangled name.
    pub fn get_instance(&self, mangled_name: &str) -> Option<&MonomorphInstance> {
        self.instances.get(mangled_name)
    }

    /// Iterate all monomorphized instances.
    pub fn instances(&self) -> impl Iterator<Item = &MonomorphInstance> {
        self.instances.values()
    }

    /// Check if a function is generic.
    pub fn is_generic_function(&self, name: &str) -> bool {
        self.generic_functions.contains_key(name)
    }

    /// Check if a type is generic.
    pub fn is_generic_type(&self, name: &str) -> bool {
        self.generic_types.contains_key(name)
    }
}

// ---------------------------------------------------------------------------
// Type substitution
// ---------------------------------------------------------------------------

/// Replace type parameter references with concrete types.
fn substitute_type(ty: &LltsType, subs: &HashMap<&str, &LltsType>) -> LltsType {
    match ty {
        // A generic type parameter reference (represented as Alias with matching name)
        LltsType::Alias { name, .. } => {
            if let Some(concrete) = subs.get(name.as_str()) {
                (*concrete).clone()
            } else {
                ty.clone()
            }
        }
        // Unknown might be an unresolved type parameter – check by name isn't possible here.
        // Generic params are resolved by the type resolver before monomorphization.

        LltsType::Array(elem) => LltsType::Array(Box::new(substitute_type(elem, subs))),

        LltsType::Tuple(elems) => {
            LltsType::Tuple(elems.iter().map(|e| substitute_type(e, subs)).collect())
        }

        LltsType::Option(inner) => LltsType::Option(Box::new(substitute_type(inner, subs))),

        LltsType::Result { ok, err } => LltsType::Result {
            ok: Box::new(substitute_type(ok, subs)),
            err: Box::new(substitute_type(err, subs)),
        },

        LltsType::Readonly(inner) => {
            LltsType::Readonly(Box::new(substitute_type(inner, subs)))
        }

        LltsType::Weak(inner) => LltsType::Weak(Box::new(substitute_type(inner, subs))),

        LltsType::Struct(s) => LltsType::Struct(StructType {
            name: s.name.clone(),
            fields: s
                .fields
                .iter()
                .map(|f| StructField {
                    name: f.name.clone(),
                    ty: substitute_type(&f.ty, subs),
                    readonly: f.readonly,
                    optional: f.optional,
                })
                .collect(),
            type_params: Vec::new(),
        }),

        LltsType::Union(u) => LltsType::Union(UnionType {
            name: u.name.clone(),
            variants: u
                .variants
                .iter()
                .map(|v| UnionVariant {
                    tag: v.tag,
                    ty: substitute_type(&v.ty, subs),
                })
                .collect(),
        }),

        LltsType::Function(f) => LltsType::Function(FunctionType {
            params: f
                .params
                .iter()
                .map(|p| FunctionParam {
                    name: p.name.clone(),
                    ty: substitute_type(&p.ty, subs),
                })
                .collect(),
            return_type: Box::new(substitute_type(&f.return_type, subs)),
            type_params: Vec::new(),
        }),

        // Generic type with nested generic – substitute through
        LltsType::Generic(g) => {
            let base = substitute_type(&g.base, subs);
            // If all type params are resolved, return the base directly
            if g.type_params.iter().all(|p| subs.contains_key(p.as_str())) {
                base
            } else {
                ty.clone()
            }
        }

        // Primitives and other types pass through unchanged
        _ => ty.clone(),
    }
}

// ---------------------------------------------------------------------------
// Name mangling
// ---------------------------------------------------------------------------

/// Generate a mangled name for a monomorphized instance.
/// Example: `identity` with `[I32]` -> `identity_i32`
fn mangle_name(name: &str, type_args: &[LltsType]) -> String {
    let mut mangled = name.to_string();
    for arg in type_args {
        mangled.push('_');
        mangled.push_str(&type_to_suffix(arg));
    }
    mangled
}

fn type_to_suffix(ty: &LltsType) -> String {
    match ty {
        LltsType::Number => "number".to_string(),
        LltsType::I8 => "i8".to_string(),
        LltsType::I16 => "i16".to_string(),
        LltsType::I32 => "i32".to_string(),
        LltsType::I64 => "i64".to_string(),
        LltsType::U8 => "u8".to_string(),
        LltsType::U16 => "u16".to_string(),
        LltsType::U32 => "u32".to_string(),
        LltsType::U64 => "u64".to_string(),
        LltsType::F32 => "f32".to_string(),
        LltsType::F64 => "f64".to_string(),
        LltsType::Boolean => "bool".to_string(),
        LltsType::String => "string".to_string(),
        LltsType::Void => "void".to_string(),
        LltsType::Never => "never".to_string(),
        LltsType::Struct(s) => s.name.clone(),
        LltsType::Array(elem) => format!("arr_{}", type_to_suffix(elem)),
        LltsType::Tuple(elems) => {
            let parts: Vec<String> = elems.iter().map(type_to_suffix).collect();
            format!("tup_{}", parts.join("_"))
        }
        LltsType::Option(inner) => format!("opt_{}", type_to_suffix(inner)),
        LltsType::Result { ok, err } => {
            format!("res_{}_{}", type_to_suffix(ok), type_to_suffix(err))
        }
        LltsType::Ref(id) => format!("ref{}", id.0),
        LltsType::Alias { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}
