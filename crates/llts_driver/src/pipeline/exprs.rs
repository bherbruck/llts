use oxc_ast::ast::*;

use llts_codegen::{
    Expr, FunctionDecl, Stmt,
    expr::BinOp,
    expr::LogicalOp,
    expr::UnaryOp,
    types::LltsType,
};

use super::context::LowerCtx;
use super::utils::{
    assignment_target_name, binding_name, expr_to_name, infer_ir_binary_type,
    ir_expr_type, lower_binop, lower_unaryop, simple_target_name,
};
use super::{codegen_type_suffix, lower_stmts, lower_ts_type_with_enums, mangle_generic_name};

pub(crate) fn lower_expr(expr: &Expression<'_>, ctx: &mut LowerCtx) -> Expr {
    match expr {
        Expression::NumericLiteral(num) => {
            // All numeric literals are f64 (TypeScript `number` = double).
            // Use `as` to get specific integer types: `1 as i32`, `255 as u8`.
            Expr::FloatLit {
                value: num.value,
                ty: LltsType::F64,
            }
        }
        Expression::NullLiteral(_) => {
            // Null literal — default to Option<f64>; the VarDecl handler
            // will re-wrap with the correct inner type when the declared type
            // is known.
            Expr::OptionNone { inner_type: LltsType::F64 }
        }
        Expression::BooleanLiteral(b) => Expr::BoolLit(b.value),
        Expression::StringLiteral(s) => Expr::StringLit(s.value.to_string()),
        Expression::TemplateLiteral(tpl) => {
            if tpl.expressions.is_empty() {
                // No interpolation — static string.
                let text: String = tpl
                    .quasis
                    .iter()
                    .map(|q| q.value.raw.as_str())
                    .collect();
                Expr::StringLit(text)
            } else {
                // Build interleaved parts: quasis[0], expr[0], quasis[1], expr[1], ...
                let mut parts: Vec<Expr> = Vec::new();
                for (i, quasi) in tpl.quasis.iter().enumerate() {
                    let text = quasi.value.raw.as_str();
                    if !text.is_empty() {
                        parts.push(Expr::StringLit(text.to_string()));
                    }
                    if i < tpl.expressions.len() {
                        parts.push(lower_expr(&tpl.expressions[i], ctx));
                    }
                }
                if parts.len() == 1 {
                    parts.remove(0)
                } else {
                    Expr::StringConcat { parts }
                }
            }
        }
        Expression::Identifier(id) => {
            let name = id.name.to_string();
            let ty = ctx.var_types.get(&name).cloned().unwrap_or(LltsType::F64);
            Expr::Var { name, ty }
        }
        Expression::BinaryExpression(bin) => {
            // Detect null comparison patterns: x !== null, x === null, null !== x, null === x
            let is_strict_eq = matches!(bin.operator, BinaryOperator::StrictEquality | BinaryOperator::Equality);
            let is_strict_ne = matches!(bin.operator, BinaryOperator::StrictInequality | BinaryOperator::Inequality);
            if is_strict_eq || is_strict_ne {
                let (var_expr, is_null_cmp) = match (&bin.left, &bin.right) {
                    (_, Expression::NullLiteral(_)) => (Some(&bin.left), true),
                    (Expression::NullLiteral(_), _) => (Some(&bin.right), true),
                    _ => (None, false),
                };
                if is_null_cmp {
                    if let Some(var_side) = var_expr {
                        let lowered_var = lower_expr(var_side, ctx);
                        let var_ty = ir_expr_type(&lowered_var);
                        if let LltsType::Option(inner) = &var_ty {
                            let is_some = Expr::OptionIsSome {
                                value: Box::new(lowered_var),
                                inner_type: *inner.clone(),
                            };
                            if is_strict_ne {
                                // x !== null  =>  is_some(x)
                                return is_some;
                            } else {
                                // x === null  =>  !is_some(x)
                                return Expr::Unary {
                                    op: UnaryOp::Not,
                                    operand: Box::new(is_some),
                                    ty: LltsType::Bool,
                                };
                            }
                        }
                    }
                }
            }
            let op = lower_binop(bin.operator);
            let lhs_expr = lower_expr(&bin.left, ctx);
            let rhs_expr = lower_expr(&bin.right, ctx);
            // Use the lowered expression types (which have proper ctx-aware types)
            let ty = infer_ir_binary_type(&lhs_expr, &rhs_expr);
            Expr::Binary { op, lhs: Box::new(lhs_expr), rhs: Box::new(rhs_expr), ty }
        }
        Expression::UnaryExpression(un) => {
            let op = lower_unaryop(un.operator);
            let operand_expr = lower_expr(&un.argument, ctx);
            let ty = ir_expr_type(&operand_expr);
            Expr::Unary { op, operand: Box::new(operand_expr), ty }
        }
        Expression::CallExpression(call) => {
            let args: Vec<Expr> = call.arguments.iter().map(|a| lower_argument(a, ctx)).collect();

            match &call.callee {
                Expression::StaticMemberExpression(member) => {
                    let obj_name = expr_to_name(&member.object);
                    let method = member.property.name.to_string();

                    if obj_name == "console" && method == "log" {
                        return Expr::Call {
                            callee: "print".to_string(),
                            args,
                            ret_type: LltsType::Void,
                        };
                    }

                    if obj_name == "Math" {
                        return Expr::Call {
                            callee: format!("Math_{method}"),
                            args,
                            ret_type: LltsType::F64,
                        };
                    }

                    Expr::MethodCall {
                        class_name: obj_name,
                        method_name: method,
                        receiver: Box::new(lower_expr(&member.object, ctx)),
                        args,
                        ret_type: LltsType::Void,
                    }
                }
                Expression::Identifier(id) => {
                    let callee = id.name.to_string();

                    // Check for generic function call with explicit type arguments
                    // e.g. identity<i32>(5), or with defaults: identity(5) when T has a default
                    if ctx.generic_fn_indices.contains_key(&callee) {
                        let concrete_types = if let Some(type_args) = &call.type_arguments {
                            // Explicit type args provided
                            let enum_names = ctx.enum_names();
                            let mut types: Vec<LltsType> = type_args
                                .params
                                .iter()
                                .map(|t| lower_ts_type_with_enums(t, &enum_names))
                                .collect();
                            // Fill in defaults for any missing trailing type args
                            if let Some(param_info) = ctx.generic_fn_params.get(&callee) {
                                while types.len() < param_info.len() {
                                    if let Some(default_ty) = &param_info[types.len()].1 {
                                        types.push(default_ty.clone());
                                    } else {
                                        break;
                                    }
                                }
                            }
                            Some(types)
                        } else if let Some(param_info) = ctx.generic_fn_params.get(&callee) {
                            // No type args — use defaults if ALL params have defaults
                            let defaults: Vec<LltsType> = param_info
                                .iter()
                                .filter_map(|(_, default, _)| default.clone())
                                .collect();
                            if defaults.len() == param_info.len() {
                                Some(defaults)
                            } else {
                                None // Not all params have defaults, can't use
                            }
                        } else {
                            None
                        };

                        if let Some(concrete_types) = concrete_types {
                            // Constraint checking: verify each concrete type is allowed
                            if let Some(param_info) = ctx.generic_fn_params.get(&callee) {
                                for (i, concrete) in concrete_types.iter().enumerate() {
                                    if i < param_info.len() && !param_info[i].2.is_empty() {
                                        let allowed = &param_info[i].2;
                                        if !allowed.contains(concrete) {
                                            eprintln!(
                                                "warning: type {} does not satisfy constraint for parameter '{}' in {}",
                                                codegen_type_suffix(concrete),
                                                param_info[i].0,
                                                callee,
                                            );
                                        }
                                    }
                                }
                            }

                            let mangled = mangle_generic_name(&callee, &concrete_types);

                            // Queue monomorphization if not already done
                            if !ctx.monomorphized.contains(&mangled) {
                                ctx.monomorphized.insert(mangled.clone());
                                ctx.pending_monomorphizations.push((
                                    callee.clone(),
                                    vec![],
                                    concrete_types.clone(),
                                    mangled.clone(),
                                ));
                            }

                            let ret_type = ctx.fn_ret_types.get(&mangled).cloned().unwrap_or(LltsType::Void);
                            return Expr::Call {
                                callee: mangled,
                                args,
                                ret_type,
                            };
                        }
                    }

                    let ret_type = ctx.fn_ret_types.get(&callee).cloned().unwrap_or(LltsType::Void);
                    Expr::Call {
                        callee,
                        args,
                        ret_type,
                    }
                }
                _ => Expr::Call {
                    callee: "<unknown>".to_string(),
                    args,
                    ret_type: LltsType::Void,
                },
            }
        }
        Expression::NewExpression(new_expr) => {
            let class_name = expr_to_name(&new_expr.callee);
            let args: Vec<Expr> = new_expr
                .arguments
                .iter()
                .map(|a| lower_argument(a, ctx))
                .collect();
            Expr::ConstructorCall {
                class_name: class_name.clone(),
                args,
                ret_type: ctx.full_struct_type(&class_name),
            }
        }
        Expression::StaticMemberExpression(member) => {
            let field_name = member.property.name.to_string();
            let obj_name = expr_to_name(&member.object);

            // Check if this is an enum variant access (e.g. Color.Red)
            if let Some(value) = ctx.lookup_enum_variant(&obj_name, &field_name) {
                return Expr::IntLit {
                    value,
                    ty: LltsType::I32,
                };
            }

            let object = Box::new(lower_expr(&member.object, ctx));

            // Try to resolve the object's struct type for field access
            if let Some(obj_type) = ctx.var_types.get(&obj_name).cloned() {
                if let LltsType::Struct { name: struct_name, .. } = &obj_type {
                    if let Some((field_index, field_type)) = ctx.lookup_field(struct_name, &field_name) {
                        return Expr::FieldAccess {
                            object,
                            object_type: obj_type.clone(),
                            field_index,
                            field_type,
                        };
                    }
                }
            }

            // Fallback: field_index 0 (best effort)
            Expr::FieldAccess {
                object,
                object_type: LltsType::F64,
                field_index: 0,
                field_type: LltsType::F64,
            }
        }
        Expression::ComputedMemberExpression(member) => {
            let arr_expr = lower_expr(&member.object, ctx);
            let idx_expr = lower_expr(&member.expression, ctx);
            let elem_type = match ir_expr_type(&arr_expr) {
                LltsType::Array(elem) => *elem,
                _ => LltsType::F64,
            };
            Expr::ArrayIndex {
                array: Box::new(arr_expr),
                index: Box::new(idx_expr),
                elem_type,
            }
        }
        Expression::AssignmentExpression(assign) => {
            let value = lower_expr(&assign.right, ctx);
            let _target = assignment_target_name(&assign.left);
            value
        }
        Expression::UpdateExpression(update) => {
            let name = simple_target_name(&update.argument);
            let ty = ctx.var_types.get(&name).cloned().unwrap_or(LltsType::F64);
            let var = Expr::Var {
                name: name.clone(),
                ty: ty.clone(),
            };
            let one = Expr::FloatLit {
                value: 1.0,
                ty: LltsType::F64,
            };
            let op = if update.operator == UpdateOperator::Increment {
                BinOp::Add
            } else {
                BinOp::Sub
            };
            Expr::Binary {
                op,
                lhs: Box::new(var),
                rhs: Box::new(one),
                ty,
            }
        }
        Expression::ParenthesizedExpression(paren) => lower_expr(&paren.expression, ctx),
        Expression::TSAsExpression(as_expr) => {
            let lowered = lower_expr(&as_expr.expression, ctx);
            let from = ir_expr_type(&lowered);
            let enum_names = ctx.enum_names();
            let to = lower_ts_type_with_enums(&as_expr.type_annotation, &enum_names);
            Expr::Cast { value: Box::new(lowered), from, to }
        }
        Expression::ArrayExpression(arr) => {
            let elements: Vec<Expr> = arr
                .elements
                .iter()
                .filter_map(|el| match el {
                    ArrayExpressionElement::SpreadElement(_) => None,
                    ArrayExpressionElement::Elision(_) => None,
                    _ => Some(lower_array_element(el, ctx)),
                })
                .collect();
            let elem_type = if let Some(first) = elements.first() {
                match first {
                    Expr::IntLit { ty, .. } => ty.clone(),
                    Expr::FloatLit { ty, .. } => ty.clone(),
                    Expr::StringLit(_) => LltsType::String,
                    Expr::BoolLit(_) => LltsType::Bool,
                    _ => LltsType::F64,
                }
            } else {
                LltsType::F64
            };
            Expr::ArrayLit {
                elem_type,
                elements,
            }
        }
        Expression::ObjectExpression(obj) => {
            let mut fields = Vec::new();
            for prop in &obj.properties {
                if let ObjectPropertyKind::ObjectProperty(p) = prop {
                    let val = lower_expr(&p.value, ctx);
                    fields.push(val);
                }
            }
            // Struct type will be patched by VarDecl handler if type annotation is present
            Expr::StructLit {
                struct_type: LltsType::Struct {
                    name: String::new(),
                    fields: vec![],
                },
                fields,
            }
        }
        Expression::ConditionalExpression(cond) => {
            let condition = lower_expr(&cond.test, ctx);
            let then_expr = lower_expr(&cond.consequent, ctx);
            let else_expr = lower_expr(&cond.alternate, ctx);
            let ty = ir_expr_type(&then_expr);
            Expr::Ternary {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                ty,
            }
        }
        Expression::LogicalExpression(log) => {
            let lhs = lower_expr(&log.left, ctx);
            let rhs = lower_expr(&log.right, ctx);
            let op = match log.operator {
                LogicalOperator::And => LogicalOp::And,
                LogicalOperator::Or => LogicalOp::Or,
                LogicalOperator::Coalesce => LogicalOp::Or, // ?? treated as || for now
            };
            Expr::Logical {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                ty: LltsType::Bool,
            }
        }
        Expression::ArrowFunctionExpression(arrow) => {
            // Generate a unique lambda name
            let lambda_name = format!("__lambda_{}", ctx.lambda_counter);
            ctx.lambda_counter += 1;

            let enum_names = ctx.enum_names();

            // Save and set up parameter scope for lowering the body
            let saved_vars = ctx.var_types.clone();

            let params: Vec<(String, LltsType)> = arrow
                .params
                .items
                .iter()
                .map(|p| {
                    let pname = binding_name(&p.pattern);
                    let pty = p
                        .type_annotation
                        .as_ref()
                        .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, &enum_names))
                        .unwrap_or(LltsType::F64);
                    let pty = match &pty {
                        LltsType::Struct { name, fields } if fields.is_empty() => {
                            ctx.full_struct_type(name)
                        }
                        other => other.clone(),
                    };
                    ctx.var_types.insert(pname.clone(), pty.clone());
                    (pname, pty)
                })
                .collect();

            let ret_type = arrow
                .return_type
                .as_ref()
                .map(|r| lower_ts_type_with_enums(&r.type_annotation, &enum_names))
                .unwrap_or(LltsType::Void);

            // Track return type for struct inference inside the body
            ctx.var_types.insert("__fn_return_type__".to_string(), ret_type.clone());

            // Lower body: expression bodies have a single expression wrapped as a
            // return; block bodies are normal statement lists.
            let body = if arrow.expression {
                // Expression body: the single statement is an ExpressionStatement
                // wrapping the return value.
                let return_expr = if let Some(stmt) = arrow.body.statements.first() {
                    match stmt {
                        Statement::ExpressionStatement(es) => lower_expr(&es.expression, ctx),
                        _ => Expr::IntLit { value: 0, ty: LltsType::I32 },
                    }
                } else {
                    Expr::IntLit { value: 0, ty: LltsType::I32 }
                };
                vec![Stmt::Return(Some(return_expr))]
            } else {
                lower_stmts(&arrow.body.statements, ctx)
            };

            ctx.var_types = saved_vars;

            // Register in fn_ret_types so call sites can resolve the return type
            ctx.fn_ret_types.insert(lambda_name.clone(), ret_type.clone());

            // Store the generated function for later emission
            ctx.pending_functions.push(FunctionDecl {
                name: lambda_name.clone(),
                params,
                ret_type,
                body,
            });

            // Return a variable reference to the lambda function name
            Expr::Var {
                name: lambda_name,
                ty: LltsType::F64, // placeholder; callers use fn_ret_types for call resolution
            }
        }
        Expression::ChainExpression(chain) => {
            // v1: treat optional chaining (?.) as regular access.
            // Unwrap the ChainElement and lower it like a normal expression.
            match &chain.expression {
                ChainElement::CallExpression(call) => {
                    // Reuse the CallExpression lowering logic
                    let args: Vec<Expr> = call.arguments.iter().map(|a| lower_argument(a, ctx)).collect();
                    match &call.callee {
                        Expression::StaticMemberExpression(member) => {
                            let obj_name = expr_to_name(&member.object);
                            let method = member.property.name.to_string();

                            if obj_name == "console" && method == "log" {
                                return Expr::Call {
                                    callee: "print".to_string(),
                                    args,
                                    ret_type: LltsType::Void,
                                };
                            }

                            if obj_name == "Math" {
                                return Expr::Call {
                                    callee: format!("Math_{method}"),
                                    args,
                                    ret_type: LltsType::F64,
                                };
                            }

                            Expr::MethodCall {
                                class_name: obj_name,
                                method_name: method,
                                receiver: Box::new(lower_expr(&member.object, ctx)),
                                args,
                                ret_type: LltsType::Void,
                            }
                        }
                        Expression::Identifier(id) => {
                            let callee = id.name.to_string();
                            let ret_type = ctx.fn_ret_types.get(&callee).cloned().unwrap_or(LltsType::Void);
                            Expr::Call {
                                callee,
                                args,
                                ret_type,
                            }
                        }
                        _ => Expr::Call {
                            callee: "<unknown>".to_string(),
                            args,
                            ret_type: LltsType::Void,
                        },
                    }
                }
                ChainElement::StaticMemberExpression(member) => {
                    let field_name = member.property.name.to_string();
                    let object = Box::new(lower_expr(&member.object, ctx));
                    let obj_name = expr_to_name(&member.object);
                    if let Some(obj_type) = ctx.var_types.get(&obj_name).cloned() {
                        if let LltsType::Struct { name: struct_name, .. } = &obj_type {
                            if let Some((field_index, field_type)) = ctx.lookup_field(struct_name, &field_name) {
                                return Expr::FieldAccess {
                                    object,
                                    object_type: obj_type.clone(),
                                    field_index,
                                    field_type,
                                };
                            }
                        }
                    }
                    Expr::FieldAccess {
                        object,
                        object_type: LltsType::F64,
                        field_index: 0,
                        field_type: LltsType::F64,
                    }
                }
                ChainElement::ComputedMemberExpression(member) => {
                    let arr_expr = lower_expr(&member.object, ctx);
                    let idx_expr = lower_expr(&member.expression, ctx);
                    let elem_type = match ir_expr_type(&arr_expr) {
                        LltsType::Array(elem) => *elem,
                        _ => LltsType::F64,
                    };
                    Expr::ArrayIndex {
                        array: Box::new(arr_expr),
                        index: Box::new(idx_expr),
                        elem_type,
                    }
                }
                _ => {
                    // TSNonNullExpression, PrivateFieldExpression — fallback
                    Expr::IntLit {
                        value: 0,
                        ty: LltsType::I32,
                    }
                }
            }
        }
        _ => {
            Expr::IntLit {
                value: 0,
                ty: LltsType::I32,
            }
        }
    }
}

pub(crate) fn lower_argument(arg: &Argument<'_>, ctx: &mut LowerCtx) -> Expr {
    match arg {
        Argument::SpreadElement(spread) => lower_expr(&spread.argument, ctx),
        _ => lower_expr(arg.to_expression(), ctx),
    }
}

pub(crate) fn lower_array_element(el: &ArrayExpressionElement<'_>, ctx: &mut LowerCtx) -> Expr {
    match el {
        ArrayExpressionElement::SpreadElement(spread) => lower_expr(&spread.argument, ctx),
        ArrayExpressionElement::Elision(_) => Expr::IntLit {
            value: 0,
            ty: LltsType::I32,
        },
        _ => lower_expr(el.to_expression(), ctx),
    }
}

/// Try to lower an expression as a Stmt::Assign (for assignment and update expressions).
/// Returns None for non-assignment expressions.
pub(crate) fn try_lower_as_assign(expr: &Expression<'_>, ctx: &mut LowerCtx) -> Option<Stmt> {
    match expr {
        Expression::AssignmentExpression(assign) => {
            // Check if the target is a field access (e.g. obj.field = value)
            if let AssignmentTarget::StaticMemberExpression(member) = &assign.left {
                let obj_name = expr_to_name(&member.object);
                let field_name = member.property.name.to_string();

                if let Some(obj_type) = ctx.var_types.get(&obj_name).cloned() {
                    if let LltsType::Struct { name: struct_name, .. } = &obj_type {
                        if let Some((field_index, field_type)) = ctx.lookup_field(struct_name, &field_name) {
                            let value = if assign.operator == AssignmentOperator::Assign {
                                lower_expr(&assign.right, ctx)
                            } else {
                                let op = match assign.operator {
                                    AssignmentOperator::Addition => BinOp::Add,
                                    AssignmentOperator::Subtraction => BinOp::Sub,
                                    AssignmentOperator::Multiplication => BinOp::Mul,
                                    AssignmentOperator::Division => BinOp::Div,
                                    AssignmentOperator::Remainder => BinOp::Rem,
                                    AssignmentOperator::ShiftLeft => BinOp::Shl,
                                    AssignmentOperator::ShiftRight => BinOp::Shr,
                                    AssignmentOperator::BitwiseAnd => BinOp::BitAnd,
                                    AssignmentOperator::BitwiseOR => BinOp::BitOr,
                                    AssignmentOperator::BitwiseXOR => BinOp::BitXor,
                                    _ => BinOp::Add,
                                };
                                let lhs = Expr::FieldAccess {
                                    object: Box::new(lower_expr(&member.object, ctx)),
                                    object_type: obj_type.clone(),
                                    field_index,
                                    field_type: field_type.clone(),
                                };
                                let rhs = lower_expr(&assign.right, ctx);
                                Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), ty: field_type.clone() }
                            };
                            return Some(Stmt::FieldAssign {
                                object_name: obj_name,
                                object_type: obj_type,
                                field_index,
                                value,
                            });
                        }
                    }
                }
            }

            let target = assignment_target_name(&assign.left);
            let value = if assign.operator == AssignmentOperator::Assign {
                lower_expr(&assign.right, ctx)
            } else {
                let op = match assign.operator {
                    AssignmentOperator::Addition => BinOp::Add,
                    AssignmentOperator::Subtraction => BinOp::Sub,
                    AssignmentOperator::Multiplication => BinOp::Mul,
                    AssignmentOperator::Division => BinOp::Div,
                    AssignmentOperator::Remainder => BinOp::Rem,
                    AssignmentOperator::ShiftLeft => BinOp::Shl,
                    AssignmentOperator::ShiftRight => BinOp::Shr,
                    AssignmentOperator::BitwiseAnd => BinOp::BitAnd,
                    AssignmentOperator::BitwiseOR => BinOp::BitOr,
                    AssignmentOperator::BitwiseXOR => BinOp::BitXor,
                    _ => BinOp::Add,
                };
                let ty = ctx.var_types.get(&target).cloned().unwrap_or(LltsType::F64);
                let lhs = Expr::Var { name: target.clone(), ty: ty.clone() };
                let rhs = lower_expr(&assign.right, ctx);
                Expr::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs), ty }
            };
            // Option wrapping: when target type is Option<T>, wrap the value
            let value = if let Some(LltsType::Option(inner)) = ctx.var_types.get(&target) {
                let already_option = matches!(ir_expr_type(&value), LltsType::Option(_));
                match value {
                    Expr::OptionNone { .. } => Expr::OptionNone { inner_type: *inner.clone() },
                    Expr::OptionSome { .. } => value,
                    _ if already_option => value,
                    _ => Expr::OptionSome { value: Box::new(value), inner_type: *inner.clone() },
                }
            } else {
                value
            };
            Some(Stmt::Assign { target, value })
        }
        Expression::UpdateExpression(update) => {
            let name = simple_target_name(&update.argument);
            let ty = ctx.var_types.get(&name).cloned().unwrap_or(LltsType::F64);
            let var = Expr::Var { name: name.clone(), ty: ty.clone() };
            let one = match &ty {
                LltsType::I8 | LltsType::I16 | LltsType::I32 | LltsType::I64 |
                LltsType::U8 | LltsType::U16 | LltsType::U32 | LltsType::U64 => {
                    Expr::IntLit { value: 1, ty: ty.clone() }
                }
                _ => Expr::FloatLit { value: 1.0, ty: ty.clone() },
            };
            let op = if update.operator == UpdateOperator::Increment {
                BinOp::Add
            } else {
                BinOp::Sub
            };
            let value = Expr::Binary { op, lhs: Box::new(var), rhs: Box::new(one), ty };
            Some(Stmt::Assign { target: name, value })
        }
        _ => None,
    }
}
