use std::collections::{HashMap, HashSet};

use oxc_ast::ast::*;

use llts_codegen::{
    EnumDecl, FunctionDecl, StructDecl,
    types::{LltsType, TypeRegistry},
};

use super::context::LowerCtx;
use super::utils::{binding_name, enum_member_name, property_key_name, ts_type_name_string};
use super::lower_stmts;

// ---------------------------------------------------------------------------
// Type lowering: TS type annotations → codegen LltsType
// ---------------------------------------------------------------------------

/// Convenience wrapper: lower a TS type without enum awareness (used in first-pass
/// functions like `lower_interface` where enums may not yet be registered).
pub(crate) fn lower_ts_type(ts_type: &TSType<'_>) -> LltsType {
    let empty = HashSet::new();
    lower_ts_type_with_enums(ts_type, &empty)
}

/// Lower a TS type annotation to `LltsType`, recognizing known enum names as `I32`.
pub(crate) fn lower_ts_type_with_enums(ts_type: &TSType<'_>, enum_names: &HashSet<String>) -> LltsType {
    match ts_type {
        TSType::TSNumberKeyword(_) => LltsType::F64,
        TSType::TSBooleanKeyword(_) => LltsType::Bool,
        TSType::TSStringKeyword(_) => LltsType::String,
        TSType::TSVoidKeyword(_) => LltsType::Void,
        TSType::TSNeverKeyword(_) => LltsType::Never,
        TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => LltsType::Void,
        TSType::TSTypeReference(ref_type) => {
            let name = ts_type_name_string(&ref_type.type_name);
            match name.as_str() {
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
                "Array" => {
                    let elem = ref_type
                        .type_arguments
                        .as_ref()
                        .and_then(|args| args.params.first())
                        .map(|t| lower_ts_type_with_enums(t, enum_names))
                        .unwrap_or(LltsType::F64);
                    LltsType::Array(Box::new(elem))
                }
                "Option" => {
                    let inner = ref_type
                        .type_arguments
                        .as_ref()
                        .and_then(|args| args.params.first())
                        .map(|t| lower_ts_type_with_enums(t, enum_names))
                        .unwrap_or(LltsType::F64);
                    LltsType::Option(Box::new(inner))
                }
                "Result" => {
                    let mut args = ref_type
                        .type_arguments
                        .as_ref()
                        .map(|a| a.params.iter().map(|t| lower_ts_type_with_enums(t, enum_names)))
                        .into_iter()
                        .flatten();
                    let ok = args.next().unwrap_or(LltsType::Void);
                    let err = args.next().unwrap_or(LltsType::Void);
                    LltsType::Result {
                        ok: Box::new(ok),
                        err: Box::new(err),
                    }
                }
                _ if enum_names.contains(&name) => LltsType::I32,
                _ => LltsType::Struct {
                    name,
                    fields: vec![],
                },
            }
        }
        TSType::TSArrayType(arr) => {
            let elem = lower_ts_type_with_enums(&arr.element_type, enum_names);
            LltsType::Array(Box::new(elem))
        }
        TSType::TSFunctionType(func) => {
            let params: Vec<LltsType> = func
                .params
                .items
                .iter()
                .map(|p| {
                    p.type_annotation
                        .as_ref()
                        .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, enum_names))
                        .unwrap_or(LltsType::F64)
                })
                .collect();
            let ret = lower_ts_type_with_enums(&func.return_type.type_annotation, enum_names);
            LltsType::Function {
                params,
                ret: Box::new(ret),
            }
        }
        TSType::TSUnionType(union) => {
            // Check if ALL variants are string literals BEFORE lowering
            // (string literals don't lower to a useful LltsType).
            let string_lits: Vec<String> = union
                .types
                .iter()
                .filter_map(|ty| match ty {
                    TSType::TSLiteralType(lit) => match &lit.literal {
                        TSLiteral::StringLiteral(s) => Some(s.value.to_string()),
                        _ => None,
                    },
                    _ => None,
                })
                .collect();
            if string_lits.len() == union.types.len() && !string_lits.is_empty() {
                // All variants are string literals — emit I32 (enum-like tag).
                return LltsType::I32;
            }

            let mut types: Vec<LltsType> = Vec::new();
            let mut has_null = false;
            for ty in &union.types {
                match ty {
                    TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => has_null = true,
                    _ => types.push(lower_ts_type_with_enums(ty, enum_names)),
                }
            }
            if has_null && types.len() == 1 {
                LltsType::Option(Box::new(types.remove(0)))
            } else if types.len() == 1 {
                types.remove(0)
            } else if !types.is_empty() && types.iter().all(|t| TypeRegistry::is_numeric(t)) {
                // All non-null variants are numeric — widen to the largest type.
                let has_float = types.iter().any(|t| TypeRegistry::is_float(t));
                if has_float {
                    // Any int + any float → widest float.
                    let max_float_width = types
                        .iter()
                        .filter(|t| TypeRegistry::is_float(t))
                        .map(|t| TypeRegistry::bit_width(t))
                        .max()
                        .unwrap_or(64);
                    let max_int_width = types
                        .iter()
                        .filter(|t| TypeRegistry::is_integer(t))
                        .map(|t| TypeRegistry::bit_width(t))
                        .max()
                        .unwrap_or(0);
                    let needed = max_float_width.max(max_int_width);
                    if needed <= 32 { LltsType::F32 } else { LltsType::F64 }
                } else {
                    // All integers — widen to the widest.
                    let has_signed = types.iter().any(|t| TypeRegistry::is_signed(t));
                    let max_width = types
                        .iter()
                        .map(|t| TypeRegistry::bit_width(t))
                        .max()
                        .unwrap_or(32);
                    // Signed + unsigned at same width → signed. Only unsigned if ALL are unsigned.
                    if has_signed {
                        match max_width {
                            w if w <= 8 => LltsType::I8,
                            w if w <= 16 => LltsType::I16,
                            w if w <= 32 => LltsType::I32,
                            _ => LltsType::I64,
                        }
                    } else {
                        match max_width {
                            w if w <= 8 => LltsType::U8,
                            w if w <= 16 => LltsType::U16,
                            w if w <= 32 => LltsType::U32,
                            _ => LltsType::U64,
                        }
                    }
                }
            } else {
                LltsType::Union {
                    name: String::new(),
                    variants: types
                        .into_iter()
                        .enumerate()
                        .map(|(i, ty)| (format!("v{i}"), ty))
                        .collect(),
                }
            }
        }
        TSType::TSParenthesizedType(paren) => lower_ts_type_with_enums(&paren.type_annotation, enum_names),
        _ => LltsType::F64,
    }
}

// ---------------------------------------------------------------------------
// Interface, class, enum, type alias lowering
// ---------------------------------------------------------------------------

pub(crate) fn lower_interface(iface: &TSInterfaceDeclaration<'_>) -> Option<StructDecl> {
    let name = iface.id.name.to_string();
    let mut fields = Vec::new();
    for member in &iface.body.body {
        if let TSSignature::TSPropertySignature(prop) = member {
            let field_name = property_key_name(&prop.key);
            let ty = prop
                .type_annotation
                .as_ref()
                .map(|ann| lower_ts_type(&ann.type_annotation))
                .unwrap_or(LltsType::F64);
            fields.push((field_name, ty));
        }
    }
    Some(StructDecl { name, fields })
}

/// Extract string literal field values from an interface declaration.
/// For `interface Circle { kind: "circle"; ... }` this returns `[("kind", "circle")]`.
pub(crate) fn extract_string_literal_fields(iface: &TSInterfaceDeclaration<'_>) -> Vec<(String, String)> {
    let mut result = Vec::new();
    for member in &iface.body.body {
        if let TSSignature::TSPropertySignature(prop) = member {
            let field_name = property_key_name(&prop.key);
            if let Some(ann) = &prop.type_annotation {
                if let TSType::TSLiteralType(lit) = &ann.type_annotation {
                    if let TSLiteral::StringLiteral(s) = &lit.literal {
                        result.push((field_name, s.value.to_string()));
                    }
                }
            }
        }
    }
    result
}

/// Extract struct fields from a class declaration (first pass).
pub(crate) fn lower_class_struct(class: &Class<'_>) -> Option<StructDecl> {
    let name = class
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_else(|| "<anonymous>".to_string());

    let mut fields = Vec::new();
    for element in &class.body.body {
        if let ClassElement::PropertyDefinition(prop) = element {
            if !prop.computed {
                let field_name = property_key_name(&prop.key);
                let ty = prop
                    .type_annotation
                    .as_ref()
                    .map(|ann| lower_ts_type(&ann.type_annotation))
                    .unwrap_or(LltsType::F64);
                fields.push((field_name, ty));
            }
        }
    }

    if fields.is_empty() {
        None
    } else {
        Some(StructDecl { name, fields })
    }
}

/// Lower class methods (second pass, with struct awareness).
pub(crate) fn lower_class_methods(class: &Class<'_>, ctx: &mut LowerCtx) -> Vec<FunctionDecl> {
    let name = class
        .id
        .as_ref()
        .map(|id| id.name.to_string())
        .unwrap_or_else(|| "<anonymous>".to_string());

    let self_type = ctx.full_struct_type(&name);
    let enum_names = ctx.enum_names();
    let mut methods = Vec::new();

    for element in &class.body.body {
        if let ClassElement::MethodDefinition(method) = element {
            let method_name = property_key_name(&method.key);
            let mangled = format!("{name}_{method_name}");

            let saved_vars = ctx.var_types.clone();
            ctx.var_types.insert("self".to_string(), self_type.clone());

            let mut params = vec![("self".to_string(), self_type.clone())];
            for param in &method.value.params.items {
                let pname = binding_name(&param.pattern);
                let pty = param
                    .type_annotation
                    .as_ref()
                    .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, &enum_names))
                    .unwrap_or(LltsType::F64);
                ctx.var_types.insert(pname.clone(), pty.clone());
                params.push((pname, pty));
            }

            let ret_type = method
                .value
                .return_type
                .as_ref()
                .map(|r| lower_ts_type_with_enums(&r.type_annotation, &enum_names))
                .unwrap_or(LltsType::Void);

            let body = method
                .value
                .body
                .as_ref()
                .map(|b| lower_stmts(&b.statements, ctx))
                .unwrap_or_default();

            ctx.var_types = saved_vars;

            methods.push(FunctionDecl {
                name: mangled,
                params,
                ret_type,
                body,
            });
        }
    }

    methods
}

/// Lower an enum declaration, returning the EnumDecl and the actual numeric
/// values for each variant (respecting explicit initializers and auto-increment).
pub(crate) fn lower_enum(decl: &TSEnumDeclaration<'_>) -> (EnumDecl, Vec<(String, i64)>) {
    let name = decl.id.name.to_string();
    let mut variants = Vec::new();
    let mut values = Vec::new();
    let mut next_value: i64 = 0;
    let mut is_string_enum = false;

    // Detect string enum: if any member has a string initializer, treat as string enum
    for m in &decl.body.members {
        if let Some(Expression::StringLiteral(_)) = &m.initializer {
            is_string_enum = true;
            break;
        }
    }

    for (idx, m) in decl.body.members.iter().enumerate() {
        let vname = enum_member_name(&m.id);

        let value = if is_string_enum {
            // String enums get sequential integer tags (strings are compile-time only)
            idx as i64
        } else {
            match &m.initializer {
                Some(Expression::NumericLiteral(num)) => {
                    let v = num.value as i64;
                    next_value = v + 1;
                    v
                }
                Some(Expression::UnaryExpression(un))
                    if un.operator == UnaryOperator::UnaryNegation =>
                {
                    // Handle negative numbers like `A = -1`
                    if let Expression::NumericLiteral(num) = &un.argument {
                        let v = -(num.value as i64);
                        next_value = v + 1;
                        v
                    } else {
                        let v = next_value;
                        next_value += 1;
                        v
                    }
                }
                None => {
                    // Auto-increment from last value
                    let v = next_value;
                    next_value += 1;
                    v
                }
                _ => {
                    // Unsupported initializer expression, fall back to auto-increment
                    let v = next_value;
                    next_value += 1;
                    v
                }
            }
        };

        variants.push((vname.clone(), LltsType::I32));
        values.push((vname, value));
    }

    (EnumDecl { name, variants }, values)
}

pub(crate) fn lower_type_alias(alias: &TSTypeAliasDeclaration<'_>) -> Option<StructDecl> {
    let name = alias.id.name.to_string();
    match &alias.type_annotation {
        TSType::TSTypeLiteral(lit) => {
            let mut fields = Vec::new();
            for member in &lit.members {
                if let TSSignature::TSPropertySignature(prop) = member {
                    let fname = property_key_name(&prop.key);
                    let ty = prop
                        .type_annotation
                        .as_ref()
                        .map(|ann| lower_ts_type(&ann.type_annotation))
                        .unwrap_or(LltsType::F64);
                    fields.push((fname, ty));
                }
            }
            Some(StructDecl { name, fields })
        }
        _ => None,
    }
}

/// Extract generic type parameter info: (name, default_type, constraint_types).
/// Constraints like `T extends i32 | f64` produce constraint_types = [I32, F64].
/// Single constraints like `T extends Num` (where `type Num = i8 | i32`) expand to [I8, I32].
pub(crate) fn extract_generic_param_info(
    type_params: &TSTypeParameterDeclaration<'_>,
    type_alias_members: &HashMap<String, Vec<LltsType>>,
) -> Vec<(String, Option<LltsType>, Vec<LltsType>)> {
    type_params
        .params
        .iter()
        .map(|p| {
            let name = p.name.name.to_string();
            let default = p.default.as_ref().map(|d| lower_ts_type(d));
            let constraints = match &p.constraint {
                Some(TSType::TSUnionType(union)) => {
                    union.types.iter().map(|t| lower_ts_type(t)).collect()
                }
                Some(TSType::TSTypeReference(ref_type)) => {
                    // Check if the constraint is a type alias for a union (e.g. `T extends Num`)
                    let ref_name = ts_type_name_string(&ref_type.type_name);
                    if let Some(members) = type_alias_members.get(&ref_name) {
                        members.clone()
                    } else {
                        vec![lower_ts_type(p.constraint.as_ref().unwrap())]
                    }
                }
                Some(constraint) => vec![lower_ts_type(constraint)],
                None => vec![],
            };
            (name, default, constraints)
        })
        .collect()
}
