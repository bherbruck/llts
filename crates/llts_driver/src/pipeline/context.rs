use std::collections::{HashMap, HashSet};

use llts_codegen::{FunctionDecl, types::LltsType};

/// Definition of a discriminated (tagged) union detected from type aliases.
/// e.g. `type Shape = Circle | Rectangle` where all variants share a `kind` field
/// with distinct string literal types.
#[derive(Debug, Clone)]
pub(crate) struct DiscriminatedUnionDef {
    /// The name of the discriminant field (e.g. "kind").
    pub(crate) discriminant_field: String,
    /// (discriminant_string_value, variant_struct_name, payload_type_without_discriminant).
    pub(crate) variants: Vec<(String, String, LltsType)>,
    /// The full union LltsType for this discriminated union.
    pub(crate) union_type: LltsType,
}

/// Lower an oxc Program AST into the codegen ProgramIR.
/// Lowering context: tracks struct definitions, variable types, and function signatures.
pub(crate) struct LowerCtx {
    /// Interface/class name -> field list (name, type).
    pub(crate) struct_defs: HashMap<String, Vec<(String, LltsType)>>,
    /// Enum name -> variant list (variant_name, numeric_value).
    pub(crate) enum_defs: HashMap<String, Vec<(String, i64)>>,
    /// Variable name -> type (populated during statement lowering).
    pub(crate) var_types: HashMap<String, LltsType>,
    /// Function name -> return type.
    pub(crate) fn_ret_types: HashMap<String, LltsType>,
    /// Function name -> parameter types (for coercing arguments at call sites).
    pub(crate) fn_param_types: HashMap<String, Vec<LltsType>>,
    /// Counter for generating unique lambda function names.
    pub(crate) lambda_counter: usize,
    /// Lambda functions generated from arrow expressions, to be appended after lowering.
    pub(crate) pending_functions: Vec<FunctionDecl>,
    /// String literal union type name -> (string_value -> integer_tag).
    /// e.g. `type Status = "pending" | "active" | "done"` -> {"Status": {"pending": 0, "active": 1, "done": 2}}
    pub(crate) string_literal_unions: HashMap<String, HashMap<String, i64>>,
    /// Type alias name -> resolved LltsType for non-struct aliases (e.g. `type Num = i32 | f64`).
    pub(crate) type_aliases: HashMap<String, LltsType>,
    /// Type alias name -> individual union member types (pre-widening).
    /// e.g. `type Num = i8 | i16 | i32 | i64` -> [I8, I16, I32, I64]
    pub(crate) type_alias_members: HashMap<String, Vec<LltsType>>,
    /// Generic function name -> index into the program body where the FunctionDeclaration lives.
    /// These functions have TSTypeParameterDeclaration and are not lowered directly.
    pub(crate) generic_fn_indices: HashMap<String, usize>,
    /// Generic function name -> list of (param_name, default_type, constraint_types).
    /// default_type is the lowered default (e.g. `T = f64` -> Some(F64)).
    /// constraint_types is the set of allowed types from `extends` (e.g. `T extends i32 | f64` -> [I32, F64]).
    pub(crate) generic_fn_params: HashMap<String, Vec<(String, Option<LltsType>, Vec<LltsType>)>>,
    /// Set of already-monomorphized specializations (mangled names) to avoid duplicates.
    pub(crate) monomorphized: HashSet<String>,
    /// Pending monomorphization requests: (generic_fn_name, type_param_names, concrete_types, mangled_name).
    pub(crate) pending_monomorphizations: Vec<(String, Vec<String>, Vec<LltsType>, String)>,
    /// Discriminated union type name -> definition.
    /// e.g. `type Shape = Circle | Rectangle` where Circle and Rectangle share a `kind` field.
    pub(crate) discriminated_unions: HashMap<String, DiscriminatedUnionDef>,
    /// (struct_name, field_name) -> string literal value.
    /// Tracks fields with string literal types for discriminated union detection.
    pub(crate) string_literal_fields: HashMap<(String, String), String>,
}

impl LowerCtx {
    pub(crate) fn new() -> Self {
        Self {
            struct_defs: HashMap::new(),
            enum_defs: HashMap::new(),
            var_types: HashMap::new(),
            fn_ret_types: HashMap::new(),
            fn_param_types: HashMap::new(),
            lambda_counter: 0,
            pending_functions: Vec::new(),
            string_literal_unions: HashMap::new(),
            type_aliases: HashMap::new(),
            type_alias_members: HashMap::new(),
            generic_fn_indices: HashMap::new(),
            generic_fn_params: HashMap::new(),
            monomorphized: HashSet::new(),
            pending_monomorphizations: Vec::new(),
            discriminated_unions: HashMap::new(),
            string_literal_fields: HashMap::new(),
        }
    }

    /// Look up a struct field by name, returning (field_index, field_type).
    pub(crate) fn lookup_field(&self, struct_name: &str, field_name: &str) -> Option<(u32, LltsType)> {
        let fields = self.struct_defs.get(struct_name)?;
        fields
            .iter()
            .enumerate()
            .find(|(_, (name, _))| name == field_name)
            .map(|(i, (_, ty))| (i as u32, ty.clone()))
    }

    /// Look up an enum variant by enum name and variant name, returning the numeric value.
    pub(crate) fn lookup_enum_variant(&self, enum_name: &str, variant_name: &str) -> Option<i64> {
        let variants = self.enum_defs.get(enum_name)?;
        variants
            .iter()
            .find(|(name, _)| name == variant_name)
            .map(|(_, value)| *value)
    }

    /// Return the set of known enum names.
    pub(crate) fn enum_names(&self) -> HashSet<String> {
        self.enum_defs.keys().cloned().collect()
    }

    /// Build a full LltsType::Struct from a struct name in struct_defs.
    pub(crate) fn full_struct_type(&self, name: &str) -> LltsType {
        if let Some(fields) = self.struct_defs.get(name) {
            LltsType::Struct {
                name: name.to_string(),
                fields: fields.clone(),
            }
        } else {
            LltsType::Struct {
                name: name.to_string(),
                fields: vec![],
            }
        }
    }
}
