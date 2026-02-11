use oxc_ast::ast::*;

use llts_codegen::{
    Expr, Stmt,
    types::LltsType,
};

use super::context::LowerCtx;
use super::utils::{binding_name, coerce_to_type, detect_null_comparison, infer_expr_type, ir_expr_type, property_key_name};
use super::{build_union_lit_from_object, lower_expr, lower_ts_type_with_enums, try_lower_as_assign, try_lower_discriminated_switch};

/// After patching a StructLit's struct_type, coerce field values to match
/// the declared field types (e.g. f64 literal → i64 IntLit).
fn coerce_struct_fields(fields: &mut Vec<Expr>, struct_type: &LltsType) {
    if let LltsType::Struct { fields: type_fields, .. } = struct_type {
        for (i, (_, field_ty)) in type_fields.iter().enumerate() {
            if i < fields.len() {
                let old = std::mem::replace(&mut fields[i], Expr::BoolLit(false));
                fields[i] = coerce_to_type(old, field_ty);
            }
        }
    }
}

pub(crate) fn lower_stmts(stmts: &[Statement<'_>], ctx: &mut LowerCtx) -> Vec<Stmt> {
    stmts.iter().flat_map(|s| lower_stmt(s, ctx)).collect()
}

pub(crate) fn lower_stmt(stmt: &Statement<'_>, ctx: &mut LowerCtx) -> Vec<Stmt> {
    match stmt {
        Statement::VariableDeclaration(decl) => {
            let mut result = Vec::new();
            for declarator in &decl.declarations {
                // Check for destructuring patterns before simple binding
                match &declarator.id {
                    BindingPattern::ObjectPattern(obj_pat) => {
                        // Object destructuring: const { x, y } = expr;
                        if let Some(init_expr) = &declarator.init {
                            let init_lowered = lower_expr(init_expr, ctx);
                            let init_ty = ir_expr_type(&init_lowered);
                            let init_ty = match &init_ty {
                                LltsType::Struct { name: sname, fields } if fields.is_empty() => {
                                    ctx.full_struct_type(sname)
                                }
                                other => other.clone(),
                            };
                            let tmp_name = format!("__destructure_tmp_{}", result.len());
                            let mut patched_init = init_lowered;
                            if let Expr::StructLit { struct_type, fields } = &mut patched_init {
                                if matches!(struct_type, LltsType::Struct { name: n, .. } if n.is_empty()) {
                                    *struct_type = init_ty.clone();
                                }
                                coerce_struct_fields(fields, struct_type);
                            }
                            ctx.var_types.insert(tmp_name.clone(), init_ty.clone());
                            result.push(Stmt::VarDecl {
                                name: tmp_name.clone(),
                                ty: init_ty.clone(),
                                init: Some(patched_init),
                            });

                            let struct_name = match &init_ty {
                                LltsType::Struct { name, .. } => name.clone(),
                                _ => String::new(),
                            };
                            for prop in &obj_pat.properties {
                                let field_name = property_key_name(&prop.key);
                                let var_name = binding_name(&prop.value);
                                let var_name = if var_name == "_" { field_name.clone() } else { var_name };

                                if let Some((field_index, field_type)) = ctx.lookup_field(&struct_name, &field_name) {
                                    ctx.var_types.insert(var_name.clone(), field_type.clone());
                                    result.push(Stmt::VarDecl {
                                        name: var_name,
                                        ty: field_type.clone(),
                                        init: Some(Expr::FieldAccess {
                                            object: Box::new(Expr::Var {
                                                name: tmp_name.clone(),
                                                ty: init_ty.clone(),
                                            }),
                                            object_type: init_ty.clone(),
                                            field_index,
                                            field_type,
                                        }),
                                    });
                                } else {
                                    ctx.var_types.insert(var_name.clone(), LltsType::F64);
                                    result.push(Stmt::VarDecl {
                                        name: var_name,
                                        ty: LltsType::F64,
                                        init: None,
                                    });
                                }
                            }
                        }
                    }
                    BindingPattern::ArrayPattern(arr_pat) => {
                        // Array destructuring: const [a, b] = expr;
                        if let Some(init_expr) = &declarator.init {
                            let init_lowered = lower_expr(init_expr, ctx);
                            let init_ty = ir_expr_type(&init_lowered);
                            let elem_type = match &init_ty {
                                LltsType::Array(elem) => *elem.clone(),
                                _ => LltsType::F64,
                            };

                            let tmp_name = format!("__destructure_tmp_{}", result.len());
                            ctx.var_types.insert(tmp_name.clone(), init_ty.clone());
                            result.push(Stmt::VarDecl {
                                name: tmp_name.clone(),
                                ty: init_ty.clone(),
                                init: Some(init_lowered),
                            });

                            for (i, elem) in arr_pat.elements.iter().enumerate() {
                                if let Some(binding) = elem {
                                    let var_name = binding_name(binding);
                                    if var_name == "_" {
                                        continue;
                                    }
                                    ctx.var_types.insert(var_name.clone(), elem_type.clone());
                                    result.push(Stmt::VarDecl {
                                        name: var_name,
                                        ty: elem_type.clone(),
                                        init: Some(Expr::ArrayIndex {
                                            array: Box::new(Expr::Var {
                                                name: tmp_name.clone(),
                                                ty: init_ty.clone(),
                                            }),
                                            index: Box::new(Expr::IntLit {
                                                value: i as i64,
                                                ty: LltsType::I64,
                                            }),
                                            elem_type: elem_type.clone(),
                                        }),
                                    });
                                }
                            }
                        }
                    }
                    _ => {
                        // Simple binding (BindingIdentifier or AssignmentPattern)
                        let name = binding_name(&declarator.id);
                        let enum_names = ctx.enum_names();
                        let ty = declarator
                            .type_annotation
                            .as_ref()
                            .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, &enum_names))
                            .or_else(|| declarator.init.as_ref().map(|e| infer_expr_type(e)))
                            .unwrap_or(LltsType::F64);
                        let ty = match &ty {
                            LltsType::Struct { name: sname, fields } if fields.is_empty() => {
                                // Check if this is a discriminated union type.
                                if let Some(du) = ctx.discriminated_unions.get(sname) {
                                    du.union_type.clone()
                                } else {
                                    ctx.full_struct_type(sname)
                                }
                            }
                            other => other.clone(),
                        };
                        ctx.var_types.insert(name.clone(), ty.clone());

                        // Check if the type is a discriminated union for object literal construction.
                        let du_name = match &ty {
                            LltsType::Union { name: un, .. } if ctx.discriminated_unions.contains_key(un) => {
                                Some(un.clone())
                            }
                            _ => None,
                        };
                        let mut init = if let (Some(du_n), Some(Expression::ObjectExpression(obj))) =
                            (&du_name, declarator.init.as_ref())
                        {
                            build_union_lit_from_object(obj, du_n, ctx)
                        } else {
                            declarator.init.as_ref().map(|e| lower_expr(e, ctx))
                        };
                        // Coerce init to the declared type (e.g. `const x: i64 = 1`)
                        init = init.map(|e| coerce_to_type(e, &ty));
                        if let Some(Expr::StructLit { struct_type, fields }) = &mut init {
                            if matches!(struct_type, LltsType::Struct { name: n, .. } if n.is_empty()) {
                                *struct_type = ty.clone();
                            }
                            coerce_struct_fields(fields, struct_type);
                        }
                        // Array element type coercion: when declared type is Array(T)
                        // and init is ArrayLit { elem_type: U } where T != U,
                        // patch elem_type to T and wrap each element in Cast.
                        if let LltsType::Array(ref declared_elem) = ty {
                            if let Some(Expr::ArrayLit { elem_type, elements }) = &mut init {
                                if *elem_type != **declared_elem {
                                    let from = elem_type.clone();
                                    *elem_type = *declared_elem.clone();
                                    // Also patch StructLit elements inside array literals
                                    if let LltsType::Struct { name: sname, .. } = &**declared_elem {
                                        for el in elements.iter_mut() {
                                            if let Expr::StructLit { struct_type, fields } = el {
                                                if matches!(struct_type, LltsType::Struct { name: n, .. } if n.is_empty()) {
                                                    *struct_type = ctx.full_struct_type(sname);
                                                }
                                                coerce_struct_fields(fields, struct_type);
                                            }
                                        }
                                    }
                                    // For numeric type mismatches, coerce each element
                                    if from != *elem_type && !matches!(*elem_type, LltsType::Struct { .. }) {
                                        let to = elem_type.clone();
                                        *elements = elements.drain(..).map(|e| {
                                            coerce_to_type(e, &to)
                                        }).collect();
                                    }
                                }
                            }
                        }
                        // Option wrapping: when declared type is Option<T>,
                        // fix up null literals and wrap non-null values.
                        // Skip wrapping if the init already produces Option<T>.
                        if let LltsType::Option(ref inner) = ty {
                            init = init.map(|e| {
                                let already_option = matches!(ir_expr_type(&e), LltsType::Option(_));
                                match e {
                                    Expr::OptionNone { .. } => Expr::OptionNone { inner_type: *inner.clone() },
                                    Expr::OptionSome { .. } => e,
                                    _ if already_option => e,
                                    _ => Expr::OptionSome { value: Box::new(e), inner_type: *inner.clone() },
                                }
                            });
                        }
                        if let Some(Expr::Var { name: ref lambda_name, .. }) = init {
                            if lambda_name.starts_with("__lambda_") {
                                let lname = lambda_name.clone();
                                if let Some(func) = ctx.pending_functions.iter_mut().find(|f| f.name == lname) {
                                    func.name = name.clone();
                                }
                                if let Some(ret) = ctx.fn_ret_types.get(&lname).cloned() {
                                    ctx.fn_ret_types.insert(name.clone(), ret);
                                }
                                continue;
                            }
                        }
                        result.push(Stmt::VarDecl { name, ty, init });
                    }
                }
            }
            result
        }
        Statement::ExpressionStatement(expr_stmt) => {
            if let Some(assign_stmt) = try_lower_as_assign(&expr_stmt.expression, ctx) {
                vec![assign_stmt]
            } else {
                vec![Stmt::Expr(lower_expr(&expr_stmt.expression, ctx))]
            }
        }
        Statement::ReturnStatement(ret) => {
            let mut expr = ret.argument.as_ref().map(|e| lower_expr(e, ctx));
            // Coerce return value to function return type
            if let Some(fn_ret) = ctx.var_types.get("__fn_return_type__").cloned() {
                expr = expr.map(|e| coerce_to_type(e, &fn_ret));
            }
            // Patch StructLit type from function return type
            if let Some(Expr::StructLit { struct_type, fields }) = &mut expr {
                if matches!(struct_type, LltsType::Struct { name: n, .. } if n.is_empty()) {
                    if let Some(fn_ret) = ctx.var_types.get("__fn_return_type__") {
                        *struct_type = fn_ret.clone();
                    }
                }
                coerce_struct_fields(fields, struct_type);
            }
            // Option wrapping for return values when function returns Option<T>
            if let Some(fn_ret) = ctx.var_types.get("__fn_return_type__").cloned() {
                if let LltsType::Option(ref inner) = fn_ret {
                    expr = expr.map(|e| {
                        let already_option = matches!(ir_expr_type(&e), LltsType::Option(_));
                        match e {
                            Expr::OptionNone { .. } => Expr::OptionNone { inner_type: *inner.clone() },
                            Expr::OptionSome { .. } => e,
                            _ if already_option => e,
                            _ => Expr::OptionSome { value: Box::new(e), inner_type: *inner.clone() },
                        }
                    });
                }
            }
            vec![Stmt::Return(expr)]
        }
        Statement::IfStatement(if_stmt) => {
            // Detect null comparison patterns for Option narrowing
            let null_narrow_info = detect_null_comparison(&if_stmt.test, ctx);
            let condition = lower_expr(&if_stmt.test, ctx);

            let then_body = {
                let saved_vars = ctx.var_types.clone();
                // If `x !== null`, narrow x to T in the then-branch
                if let Some((ref var_name, ref inner_ty, true)) = null_narrow_info {
                    ctx.var_types.insert(var_name.clone(), inner_ty.clone());
                }
                let mut stmts = match &if_stmt.consequent {
                    Statement::BlockStatement(block) => lower_stmts(&block.body, ctx),
                    other => lower_stmt(other, ctx),
                };
                // Prepend an unwrap assignment if narrowing (x !== null in then)
                if let Some((ref var_name, ref inner_ty, true)) = null_narrow_info {
                    let opt_ty = LltsType::Option(Box::new(inner_ty.clone()));
                    stmts.insert(0, Stmt::VarDecl {
                        name: var_name.clone(),
                        ty: inner_ty.clone(),
                        init: Some(Expr::OptionUnwrap {
                            value: Box::new(Expr::Var { name: var_name.clone(), ty: opt_ty }),
                            inner_type: inner_ty.clone(),
                        }),
                    });
                }
                ctx.var_types = saved_vars;
                stmts
            };

            let else_body = if_stmt.alternate.as_ref().map(|alt| {
                let saved_vars = ctx.var_types.clone();
                // If `x === null`, narrow x to T in the else-branch
                if let Some((ref var_name, ref inner_ty, false)) = null_narrow_info {
                    ctx.var_types.insert(var_name.clone(), inner_ty.clone());
                }
                let mut stmts = match alt {
                    Statement::BlockStatement(block) => lower_stmts(&block.body, ctx),
                    other => lower_stmt(other, ctx),
                };
                // Prepend an unwrap assignment if narrowing (x === null -> unwrap in else)
                if let Some((ref var_name, ref inner_ty, false)) = null_narrow_info {
                    let opt_ty = LltsType::Option(Box::new(inner_ty.clone()));
                    stmts.insert(0, Stmt::VarDecl {
                        name: var_name.clone(),
                        ty: inner_ty.clone(),
                        init: Some(Expr::OptionUnwrap {
                            value: Box::new(Expr::Var { name: var_name.clone(), ty: opt_ty }),
                            inner_type: inner_ty.clone(),
                        }),
                    });
                }
                ctx.var_types = saved_vars;
                stmts
            });

            vec![Stmt::If {
                condition,
                then_body,
                else_body,
            }]
        }
        Statement::WhileStatement(while_stmt) => {
            let condition = lower_expr(&while_stmt.test, ctx);
            let body = match &while_stmt.body {
                Statement::BlockStatement(block) => lower_stmts(&block.body, ctx),
                other => lower_stmt(other, ctx),
            };
            vec![Stmt::While { condition, body }]
        }
        Statement::ForStatement(for_stmt) => {
            let init = for_stmt.init.as_ref().and_then(|i| match i {
                ForStatementInit::VariableDeclaration(decl) => {
                    let declarator = &decl.declarations[0];
                    let name = binding_name(&declarator.id);
                    let ty = declarator
                        .init
                        .as_ref()
                        .map(|e| infer_expr_type(e))
                        .unwrap_or(LltsType::F64);
                    ctx.var_types.insert(name.clone(), ty.clone());
                    let init_expr = declarator.init.as_ref().map(|e| lower_expr(e, ctx));
                    Some(Box::new(Stmt::VarDecl {
                        name,
                        ty,
                        init: init_expr,
                    }))
                }
                _ => None,
            });
            let condition = for_stmt.test.as_ref().map(|e| lower_expr(e, ctx));
            let update = for_stmt.update.as_ref().map(|e| {
                Box::new(try_lower_as_assign(e, ctx).unwrap_or_else(|| Stmt::Expr(lower_expr(e, ctx))))
            });
            let body = match &for_stmt.body {
                Statement::BlockStatement(block) => lower_stmts(&block.body, ctx),
                other => lower_stmt(other, ctx),
            };
            vec![Stmt::For {
                init,
                condition,
                update,
                body,
            }]
        }
        Statement::ForOfStatement(forof) => {
            let elem_name = match &forof.left {
                ForStatementLeft::VariableDeclaration(decl) => {
                    binding_name(&decl.declarations[0].id)
                }
                _ => "_".to_string(),
            };
            let iterable = lower_expr(&forof.right, ctx);
            let elem_type = match ir_expr_type(&iterable) {
                LltsType::Array(inner) => *inner,
                _ => LltsType::F64,
            };
            // Resolve empty struct types to full struct types
            let elem_type = match &elem_type {
                LltsType::Struct { name, fields } if fields.is_empty() => {
                    ctx.full_struct_type(name)
                }
                other => other.clone(),
            };
            ctx.var_types.insert(elem_name.clone(), elem_type.clone());
            let body = match &forof.body {
                Statement::BlockStatement(block) => lower_stmts(&block.body, ctx),
                other => lower_stmt(other, ctx),
            };
            vec![Stmt::ForOf {
                elem_name,
                elem_type,
                iterable,
                body,
            }]
        }
        Statement::SwitchStatement(switch) => {
            // Check for discriminated union switch: switch (s.kind)
            if let Some(result) = try_lower_discriminated_switch(switch, ctx) {
                return result;
            }
            let discriminant = lower_expr(&switch.discriminant, ctx);
            let cases = switch
                .cases
                .iter()
                .map(|case| {
                    let test = case.test.as_ref().map(|t| lower_expr(t, ctx));
                    let body = lower_stmts(&case.consequent, ctx);
                    (test, body)
                })
                .collect();
            vec![Stmt::Switch {
                discriminant,
                cases,
            }]
        }
        Statement::BreakStatement(_) => vec![Stmt::Break],
        Statement::ContinueStatement(_) => vec![Stmt::Continue],
        Statement::BlockStatement(block) => vec![Stmt::Block(lower_stmts(&block.body, ctx))],
        Statement::ThrowStatement(throw) => {
            vec![Stmt::Throw(lower_expr(&throw.argument, ctx))]
        }
        Statement::TryStatement(try_stmt) => {
            let try_body = lower_stmts(&try_stmt.block.body, ctx);
            let (catch_param, catch_body) = if let Some(handler) = &try_stmt.handler {
                let param = handler.param.as_ref().map(|p| binding_name(&p.pattern));
                let body = lower_stmts(&handler.body.body, ctx);
                (param, body)
            } else {
                (None, Vec::new())
            };
            vec![Stmt::TryCatch {
                try_body,
                catch_param,
                catch_body,
            }]
        }
        _ => vec![],
    }
}
