use std::collections::HashSet;

use oxc_ast::ast::*;

use llts_codegen::{
    EnumDecl, Expr, StructDecl, Stmt,
    types::LltsType,
};

use super::context::{DiscriminatedUnionDef, LowerCtx};
use super::utils::{expr_to_name, property_key_name, ts_type_name_string};
use super::{lower_expr, lower_stmts};

/// Build an Expr::UnionLit from an ObjectExpression when the target type is a discriminated union.
/// Returns None if the object doesn't match a valid variant.
pub(crate) fn build_union_lit_from_object(
    obj: &ObjectExpression<'_>,
    du_name: &str,
    ctx: &mut LowerCtx,
) -> Option<Expr> {
    let du = ctx.discriminated_unions.get(du_name)?.clone();

    // Extract field names and values from the object literal.
    let mut field_names: Vec<String> = Vec::new();
    let mut field_values: Vec<&Expression<'_>> = Vec::new();
    for prop in &obj.properties {
        if let ObjectPropertyKind::ObjectProperty(p) = prop {
            let fname = property_key_name(&p.key);
            field_names.push(fname);
            field_values.push(&p.value);
        }
    }

    // Find the discriminant field value.
    let disc_idx = field_names.iter().position(|n| n == &du.discriminant_field)?;
    let disc_value = match field_values[disc_idx] {
        Expression::StringLiteral(s) => s.value.to_string(),
        _ => return None,
    };

    // Look up which variant this discriminant value maps to.
    let (tag, _variant_name, payload_type) = du.variants.iter().enumerate().find_map(|(i, (dv, vn, pt))| {
        if dv == &disc_value {
            Some((i as u32, vn.clone(), pt.clone()))
        } else {
            None
        }
    })?;

    // Build the payload StructLit (without the discriminant field).
    let payload_fields: Vec<Expr> = field_names
        .iter()
        .zip(field_values.iter())
        .filter(|(name, _)| name.as_str() != du.discriminant_field)
        .map(|(_, val)| lower_expr(val, ctx))
        .collect();

    let payload_expr = Expr::StructLit {
        struct_type: payload_type,
        fields: payload_fields,
    };

    Some(Expr::UnionLit {
        tag,
        payload: Box::new(payload_expr),
        union_type: du.union_type.clone(),
    })
}

/// Detect a discriminated union from a type alias like `type Shape = Circle | Rectangle`.
/// If all union members are struct type references that share a common string-literal
/// discriminant field, register a DiscriminatedUnionDef and emit an EnumDecl for codegen.
pub(crate) fn detect_discriminated_union(
    alias: &TSTypeAliasDeclaration<'_>,
    ctx: &mut LowerCtx,
    structs: &mut Vec<StructDecl>,
    enums: &mut Vec<EnumDecl>,
) {
    let union_name = alias.id.name.to_string();

    // Only process TSUnionType annotations.
    let union_type = match &alias.type_annotation {
        TSType::TSUnionType(u) => u,
        _ => return,
    };

    // Collect variant struct names from the union members.
    let mut variant_struct_names: Vec<String> = Vec::new();
    for member in &union_type.types {
        if let TSType::TSTypeReference(ref_type) = member {
            let name = ts_type_name_string(&ref_type.type_name);
            if ctx.struct_defs.contains_key(&name) {
                variant_struct_names.push(name);
            } else {
                return; // Not all members are known structs - bail.
            }
        } else {
            return; // Non-reference member - bail.
        }
    }

    if variant_struct_names.len() < 2 {
        return;
    }

    // Find a common field name that has string literal values in all variants.
    let first = &variant_struct_names[0];
    let first_fields: Vec<String> = ctx
        .string_literal_fields
        .keys()
        .filter(|(sn, _)| sn == first)
        .map(|(_, fn_name)| fn_name.clone())
        .collect();

    let mut discriminant_field: Option<String> = None;
    for field_name in &first_fields {
        let mut all_have_it = true;
        let mut values_unique = HashSet::new();
        for struct_name in &variant_struct_names {
            if let Some(value) = ctx.string_literal_fields.get(&(struct_name.clone(), field_name.clone())) {
                if !values_unique.insert(value.clone()) {
                    all_have_it = false;
                    break;
                }
            } else {
                all_have_it = false;
                break;
            }
        }
        if all_have_it && values_unique.len() == variant_struct_names.len() {
            discriminant_field = Some(field_name.clone());
            break;
        }
    }

    let discriminant_field = match discriminant_field {
        Some(f) => f,
        None => return,
    };

    // Build the DiscriminatedUnionDef.
    // For each variant, create a payload struct type WITHOUT the discriminant field.
    let mut variants: Vec<(String, String, LltsType)> = Vec::new();
    let mut enum_variants: Vec<(String, LltsType)> = Vec::new();
    for (tag, struct_name) in variant_struct_names.iter().enumerate() {
        let disc_value = ctx
            .string_literal_fields
            .get(&(struct_name.clone(), discriminant_field.clone()))
            .unwrap()
            .clone();

        // Build the payload type: the full struct without the discriminant field.
        let full_fields = ctx.struct_defs.get(struct_name).cloned().unwrap_or_default();
        let payload_fields: Vec<(String, LltsType)> = full_fields
            .into_iter()
            .filter(|(name, _)| name != &discriminant_field)
            .collect();

        let payload_struct_name = format!("{union_name}_{struct_name}");
        let payload_type = LltsType::Struct {
            name: payload_struct_name.clone(),
            fields: payload_fields.clone(),
        };

        // Register the payload struct so codegen knows about it.
        ctx.struct_defs.insert(payload_struct_name.clone(), payload_fields.clone());
        structs.push(StructDecl {
            name: payload_struct_name,
            fields: payload_fields,
        });

        variants.push((disc_value, struct_name.clone(), payload_type.clone()));
        enum_variants.push((format!("v{tag}"), payload_type));
    }

    let full_union_type = LltsType::Union {
        name: union_name.clone(),
        variants: enum_variants.clone(),
    };

    let def = DiscriminatedUnionDef {
        discriminant_field,
        variants,
        union_type: full_union_type,
    };
    ctx.discriminated_unions.insert(union_name.clone(), def);

    // Emit an EnumDecl so codegen registers the union type.
    enums.push(EnumDecl {
        name: union_name,
        variants: enum_variants,
    });
}

/// Try to lower a switch statement as a discriminated union switch.
/// Detects `switch (s.kind) { case "circle": ... }` where `s` is a discriminated union.
/// Returns None if the pattern doesn't match, so the caller falls back to normal switch lowering.
pub(crate) fn try_lower_discriminated_switch(
    switch: &SwitchStatement<'_>,
    ctx: &mut LowerCtx,
) -> Option<Vec<Stmt>> {
    // The discriminant must be a member expression: s.kind
    let (var_name, field_name) = match &switch.discriminant {
        Expression::StaticMemberExpression(member) => {
            let obj = expr_to_name(&member.object);
            let field = member.property.name.to_string();
            (obj, field)
        }
        _ => return None,
    };

    // The variable must be a discriminated union type.
    let var_ty = ctx.var_types.get(&var_name)?.clone();
    let du_type_name = match &var_ty {
        LltsType::Union { name, .. } => name.clone(),
        _ => return None,
    };
    let du = ctx.discriminated_unions.get(&du_type_name)?.clone();

    // The field must be the discriminant field.
    if field_name != du.discriminant_field {
        return None;
    }

    // Save the original union value in a unique temp variable so it survives
    // across case blocks (the codegen does not scope switch case variables).
    let union_tmp = format!("__du_tmp_{var_name}");
    let mut preamble = vec![Stmt::VarDecl {
        name: union_tmp.clone(),
        ty: var_ty.clone(),
        init: Some(Expr::Var {
            name: var_name.clone(),
            ty: var_ty.clone(),
        }),
    }];

    // Build a switch on the tag (field 0 of the saved union temp).
    let discriminant = Expr::FieldAccess {
        object: Box::new(Expr::Var {
            name: union_tmp.clone(),
            ty: var_ty.clone(),
        }),
        object_type: var_ty.clone(),
        field_index: 0,
        field_type: LltsType::I32,
    };

    let mut cases: Vec<(Option<Expr>, Vec<Stmt>)> = Vec::new();

    for case in &switch.cases {
        if let Some(test) = &case.test {
            // Extract the string literal from the case test.
            let case_str = match test {
                Expression::StringLiteral(s) => s.value.to_string(),
                _ => {
                    // Not a string literal case - fall back to normal switch.
                    return None;
                }
            };

            // Map string value to tag index.
            let tag = du.variants.iter().position(|(dv, _, _)| dv == &case_str)?;

            let test_expr = Expr::IntLit {
                value: tag as i64,
                ty: LltsType::I32,
            };

            // Narrow the variable's type in this case block.
            let saved_var_types = ctx.var_types.clone();

            let (_, _variant_struct_name, payload_type) = &du.variants[tag];

            // Extract the payload from the saved union temp (field 1 of the union struct).
            let extract_expr = Expr::FieldAccess {
                object: Box::new(Expr::Var {
                    name: union_tmp.clone(),
                    ty: var_ty.clone(),
                }),
                object_type: var_ty.clone(),
                field_index: 1,
                field_type: payload_type.clone(),
            };

            // Rebind the variable to the extracted payload struct.
            // This shadowing VarDecl will create a new alloca with the narrowed type.
            ctx.var_types.insert(var_name.clone(), payload_type.clone());

            // Lower the case body with narrowed types.
            let mut body = lower_stmts(&case.consequent, ctx);

            // Prepend the payload extraction VarDecl.
            body.insert(0, Stmt::VarDecl {
                name: var_name.clone(),
                ty: payload_type.clone(),
                init: Some(extract_expr),
            });

            ctx.var_types = saved_var_types;

            cases.push((Some(test_expr), body));
        } else {
            // Default case.
            let body = lower_stmts(&case.consequent, ctx);
            cases.push((None, body));
        }
    }

    preamble.push(Stmt::Switch {
        discriminant,
        cases,
    });
    Some(preamble)
}
