use oxc_ast::ast::*;

use llts_codegen::{
    Expr,
    expr::BinOp,
    expr::UnaryOp,
    types::LltsType,
};

use super::context::LowerCtx;

pub(crate) fn lower_binop(op: BinaryOperator) -> BinOp {
    match op {
        BinaryOperator::Addition => BinOp::Add,
        BinaryOperator::Subtraction => BinOp::Sub,
        BinaryOperator::Multiplication => BinOp::Mul,
        BinaryOperator::Division => BinOp::Div,
        BinaryOperator::Remainder => BinOp::Rem,
        BinaryOperator::LessThan => BinOp::Lt,
        BinaryOperator::LessEqualThan => BinOp::Le,
        BinaryOperator::GreaterThan => BinOp::Gt,
        BinaryOperator::GreaterEqualThan => BinOp::Ge,
        BinaryOperator::Equality | BinaryOperator::StrictEquality => BinOp::Eq,
        BinaryOperator::Inequality | BinaryOperator::StrictInequality => BinOp::Ne,
        BinaryOperator::BitwiseAnd => BinOp::BitAnd,
        BinaryOperator::BitwiseOR => BinOp::BitOr,
        BinaryOperator::BitwiseXOR => BinOp::BitXor,
        BinaryOperator::ShiftLeft => BinOp::Shl,
        BinaryOperator::ShiftRight => BinOp::Shr,
        BinaryOperator::ShiftRightZeroFill => BinOp::Shr,
        BinaryOperator::Exponential => BinOp::Mul, // placeholder
        _ => BinOp::Add,
    }
}

pub(crate) fn lower_unaryop(op: UnaryOperator) -> UnaryOp {
    match op {
        UnaryOperator::UnaryNegation => UnaryOp::Neg,
        UnaryOperator::LogicalNot => UnaryOp::Not,
        UnaryOperator::BitwiseNot => UnaryOp::BitNot,
        _ => UnaryOp::Neg,
    }
}

pub(crate) fn infer_expr_type(expr: &Expression<'_>) -> LltsType {
    match expr {
        Expression::NumericLiteral(_) => LltsType::F64,
        Expression::BooleanLiteral(_) => LltsType::Bool,
        Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => LltsType::String,
        _ => LltsType::F64,
    }
}

/// Returns the *operand* type for a binary expression based on lowered IR expressions.
/// This is more accurate than `infer_binary_type` because it uses context-resolved types.
pub(crate) fn infer_ir_binary_type(lhs: &Expr, rhs: &Expr) -> LltsType {
    let lt = ir_expr_type(lhs);
    let rt = ir_expr_type(rhs);

    if matches!(lt, LltsType::String) || matches!(rt, LltsType::String) {
        return LltsType::String;
    }
    if matches!(lt, LltsType::F64) || matches!(rt, LltsType::F64) {
        return LltsType::F64;
    }
    lt
}

/// Detect null comparison patterns in an if-condition.
/// Returns Some((variable_name, inner_type, is_not_null_check)) if the
/// condition is `x !== null` or `x === null` where x is Option<T>.
///   is_not_null_check = true  means `x !== null` (narrow in then-branch)
///   is_not_null_check = false means `x === null` (narrow in else-branch)
pub(crate) fn detect_null_comparison(expr: &Expression<'_>, ctx: &LowerCtx) -> Option<(String, LltsType, bool)> {
    let bin = match expr {
        Expression::BinaryExpression(b) => b,
        _ => return None,
    };
    let is_ne = matches!(bin.operator, BinaryOperator::StrictInequality | BinaryOperator::Inequality);
    let is_eq = matches!(bin.operator, BinaryOperator::StrictEquality | BinaryOperator::Equality);
    if !is_ne && !is_eq {
        return None;
    }
    // Determine which side is null and which is the variable
    let var_name = match (&bin.left, &bin.right) {
        (Expression::Identifier(id), Expression::NullLiteral(_)) => id.name.to_string(),
        (Expression::NullLiteral(_), Expression::Identifier(id)) => id.name.to_string(),
        _ => return None,
    };
    // Check if the variable is Option<T>
    let var_ty = ctx.var_types.get(&var_name)?;
    if let LltsType::Option(inner) = var_ty {
        Some((var_name, *inner.clone(), is_ne))
    } else {
        None
    }
}

pub(crate) fn ir_expr_type(expr: &Expr) -> LltsType {
    match expr {
        Expr::IntLit { ty, .. } => ty.clone(),
        Expr::FloatLit { ty, .. } => ty.clone(),
        Expr::BoolLit(_) => LltsType::Bool,
        Expr::StringLit(_) => LltsType::String,
        Expr::Var { ty, .. } => ty.clone(),
        Expr::Binary { ty, .. } => ty.clone(),
        Expr::Unary { ty, .. } => ty.clone(),
        Expr::Call { ret_type, .. } => ret_type.clone(),
        Expr::FieldAccess { field_type, .. } => field_type.clone(),
        Expr::StructLit { struct_type, .. } => struct_type.clone(),
        Expr::Ternary { ty, .. } => ty.clone(),
        Expr::StringConcat { .. } => LltsType::String,
        Expr::Logical { ty, .. } => ty.clone(),
        Expr::UnionLit { union_type, .. } => union_type.clone(),
        Expr::OptionNone { inner_type } => LltsType::Option(Box::new(inner_type.clone())),
        Expr::OptionSome { inner_type, .. } => LltsType::Option(Box::new(inner_type.clone())),
        Expr::OptionIsSome { .. } => LltsType::Bool,
        Expr::OptionUnwrap { inner_type, .. } => inner_type.clone(),
        _ => LltsType::F64,
    }
}

pub(crate) fn binding_name(pattern: &BindingPattern<'_>) -> String {
    match pattern {
        BindingPattern::BindingIdentifier(id) => id.name.to_string(),
        _ => "_".to_string(),
    }
}

pub(crate) fn property_key_name(key: &PropertyKey<'_>) -> String {
    match key {
        PropertyKey::StaticIdentifier(id) => id.name.to_string(),
        _ => "<computed>".to_string(),
    }
}

pub(crate) fn enum_member_name(name: &TSEnumMemberName<'_>) -> String {
    match name {
        TSEnumMemberName::Identifier(id) => id.name.to_string(),
        TSEnumMemberName::String(s) => s.value.to_string(),
        _ => "<computed>".to_string(),
    }
}

pub(crate) fn expr_to_name(expr: &Expression<'_>) -> String {
    match expr {
        Expression::Identifier(id) => id.name.to_string(),
        _ => "<expr>".to_string(),
    }
}

pub(crate) fn assignment_target_name(target: &AssignmentTarget<'_>) -> String {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => id.name.to_string(),
        _ => "_".to_string(),
    }
}

pub(crate) fn simple_target_name(target: &SimpleAssignmentTarget<'_>) -> String {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => id.name.to_string(),
        _ => "_".to_string(),
    }
}

pub(crate) fn ts_type_name_string(name: &TSTypeName<'_>) -> String {
    match name {
        TSTypeName::IdentifierReference(id) => id.name.to_string(),
        TSTypeName::QualifiedName(q) => {
            let left = ts_type_name_string(&q.left);
            let right = q.right.name.to_string();
            format!("{left}.{right}")
        }
        TSTypeName::ThisExpression(_) => "this".to_string(),
    }
}
