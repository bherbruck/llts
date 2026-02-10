use std::collections::{HashMap, HashSet};

use oxc_ast::ast::*;

use llts_codegen::{
    FunctionDecl,
    types::LltsType,
};

use super::context::LowerCtx;
use super::utils::{binding_name, ts_type_name_string};
use super::{lower_stmts, lower_ts_type_with_enums};

/// Lower a generic function with concrete type substitutions, producing a specialized FunctionDecl.
pub(crate) fn lower_generic_function(
    func: &Function<'_>,
    ctx: &mut LowerCtx,
    mangled_name: &str,
    generics: &HashMap<String, LltsType>,
) -> Option<FunctionDecl> {
    let enum_names = ctx.enum_names();
    let saved_vars = ctx.var_types.clone();
    let params: Vec<(String, LltsType)> = func
        .params
        .items
        .iter()
        .map(|p| {
            let pname = binding_name(&p.pattern);
            let pty = p
                .type_annotation
                .as_ref()
                .map(|ann| lower_ts_type_with_generics(&ann.type_annotation, &enum_names, generics))
                .unwrap_or(LltsType::F64);
            let pty = match &pty {
                LltsType::Struct { name, fields } if fields.is_empty() => ctx.full_struct_type(name),
                other => other.clone(),
            };
            ctx.var_types.insert(pname.clone(), pty.clone());
            (pname, pty)
        })
        .collect();
    let ret_type = func
        .return_type
        .as_ref()
        .map(|r| lower_ts_type_with_generics(&r.type_annotation, &enum_names, generics))
        .unwrap_or(LltsType::Void);
    ctx.var_types.insert("__fn_return_type__".to_string(), ret_type.clone());
    let body = func
        .body
        .as_ref()
        .map(|b| lower_stmts(&b.statements, ctx))
        .unwrap_or_default();
    ctx.var_types = saved_vars;
    Some(FunctionDecl {
        name: mangled_name.to_string(),
        params,
        ret_type,
        body,
    })
}

/// Lower a TS type annotation with generic type parameter substitution.
pub(crate) fn lower_ts_type_with_generics(
    ts_type: &TSType<'_>,
    enum_names: &HashSet<String>,
    generics: &HashMap<String, LltsType>,
) -> LltsType {
    match ts_type {
        TSType::TSTypeReference(ref_type) => {
            let name = ts_type_name_string(&ref_type.type_name);
            if let Some(concrete) = generics.get(&name) {
                return concrete.clone();
            }
            match name.as_str() {
                "Array" => {
                    let elem = ref_type
                        .type_arguments
                        .as_ref()
                        .and_then(|args| args.params.first())
                        .map(|t| lower_ts_type_with_generics(t, enum_names, generics))
                        .unwrap_or(LltsType::F64);
                    return LltsType::Array(Box::new(elem));
                }
                "Option" => {
                    let inner = ref_type
                        .type_arguments
                        .as_ref()
                        .and_then(|args| args.params.first())
                        .map(|t| lower_ts_type_with_generics(t, enum_names, generics))
                        .unwrap_or(LltsType::F64);
                    return LltsType::Option(Box::new(inner));
                }
                "Result" => {
                    let mut args_iter = ref_type
                        .type_arguments
                        .as_ref()
                        .map(|a| a.params.iter().map(|t| lower_ts_type_with_generics(t, enum_names, generics)))
                        .into_iter()
                        .flatten();
                    let ok = args_iter.next().unwrap_or(LltsType::Void);
                    let err = args_iter.next().unwrap_or(LltsType::Void);
                    return LltsType::Result { ok: Box::new(ok), err: Box::new(err) };
                }
                _ => {}
            }
            lower_ts_type_with_enums(ts_type, enum_names)
        }
        TSType::TSArrayType(arr) => {
            LltsType::Array(Box::new(lower_ts_type_with_generics(&arr.element_type, enum_names, generics)))
        }
        TSType::TSUnionType(union) => {
            let mut types: Vec<LltsType> = Vec::new();
            let mut has_null = false;
            for ty in &union.types {
                match ty {
                    TSType::TSNullKeyword(_) | TSType::TSUndefinedKeyword(_) => has_null = true,
                    _ => types.push(lower_ts_type_with_generics(ty, enum_names, generics)),
                }
            }
            if has_null && types.len() == 1 {
                LltsType::Option(Box::new(types.remove(0)))
            } else if types.len() == 1 {
                types.remove(0)
            } else {
                LltsType::Union {
                    name: String::new(),
                    variants: types.into_iter().enumerate().map(|(i, ty)| (format!("v{i}"), ty)).collect(),
                }
            }
        }
        TSType::TSParenthesizedType(paren) => {
            lower_ts_type_with_generics(&paren.type_annotation, enum_names, generics)
        }
        _ => lower_ts_type_with_enums(ts_type, enum_names),
    }
}

/// Mangle a generic function name with concrete type arguments.
pub(crate) fn mangle_generic_name(name: &str, type_args: &[LltsType]) -> String {
    let mut mangled = name.to_string();
    for arg in type_args {
        mangled.push('$');
        mangled.push_str(&codegen_type_suffix(arg));
    }
    mangled
}

pub(crate) fn codegen_type_suffix(ty: &LltsType) -> String {
    match ty {
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
        LltsType::Bool => "bool".to_string(),
        LltsType::String => "string".to_string(),
        LltsType::Void => "void".to_string(),
        LltsType::Struct { name, .. } => name.clone(),
        LltsType::Array(elem) => format!("arr_{}", codegen_type_suffix(elem)),
        LltsType::Option(inner) => format!("opt_{}", codegen_type_suffix(inner)),
        _ => "unknown".to_string(),
    }
}
