use std::collections::HashMap;

use oxc_ast::ast::*;
use oxc_span::Span;

use crate::types::LltsType;

// ---------------------------------------------------------------------------
// Ownership model
// ---------------------------------------------------------------------------

/// How a value is owned at a given program point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    /// Value lives on the stack and is copied on assignment.
    /// Applies to primitives and small structs.
    Stack,
    /// Value is heap-allocated with reference counting.
    /// Applies to strings, arrays, large structs, closures.
    Rc,
    /// Value has been moved and can no longer be used.
    Moved,
}

/// How a variable is used in a given context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Usage {
    /// Read-only access (no mutation, no escaping).
    Read,
    /// Mutating access (e.g. `arr.push(...)`).
    Mutate,
    /// Value escapes the current scope (returned, stored in collection).
    Escape,
}

/// Ownership annotation for a single variable binding.
#[derive(Debug, Clone)]
pub struct OwnershipInfo {
    pub name: String,
    pub span: Span,
    pub ty: LltsType,
    pub ownership: Ownership,
    /// Whether this variable has been moved (use-after-move is an error).
    pub moved: bool,
}

/// Ownership annotation for a function parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamOwnership {
    /// Callee borrows immutably (no RC bump needed).
    Borrow,
    /// Callee borrows mutably (no RC bump, but exclusive access).
    MutableBorrow,
    /// Callee takes ownership (RC transfer or move).
    Owned,
}

/// Result of the ownership analysis pass for a single function.
#[derive(Debug, Clone)]
pub struct FunctionOwnership {
    pub name: String,
    pub params: Vec<(String, ParamOwnership)>,
    pub locals: Vec<OwnershipInfo>,
}

// ---------------------------------------------------------------------------
// Ownership errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OwnershipError {
    pub span: Span,
    pub kind: OwnershipErrorKind,
}

#[derive(Debug, Clone)]
pub enum OwnershipErrorKind {
    /// Variable used after it was moved.
    UseAfterMove { name: String },
    /// Attempt to mutate a variable that is currently borrowed.
    MutateWhileBorrowed { name: String },
}

impl std::fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            OwnershipErrorKind::UseAfterMove { name } => {
                write!(f, "use of moved variable `{name}`")
            }
            OwnershipErrorKind::MutateWhileBorrowed { name } => {
                write!(f, "cannot mutate `{name}` while it is borrowed")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ownership analyzer
// ---------------------------------------------------------------------------

pub struct OwnershipAnalyzer {
    /// Variable name -> OwnershipInfo for current scope.
    variables: HashMap<String, OwnershipInfo>,
    errors: Vec<OwnershipError>,
}

impl OwnershipAnalyzer {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Determine ownership for a type.
    pub fn ownership_for_type(ty: &LltsType) -> Ownership {
        if ty.is_primitive() || ty.is_small_struct() {
            Ownership::Stack
        } else {
            Ownership::Rc
        }
    }

    /// Determine how a function parameter should be passed based on usage analysis.
    pub fn infer_param_ownership(ty: &LltsType, usage: Usage) -> ParamOwnership {
        match usage {
            Usage::Read => ParamOwnership::Borrow,
            Usage::Mutate => ParamOwnership::MutableBorrow,
            Usage::Escape => {
                if ty.is_primitive() || ty.is_small_struct() {
                    // Primitives are always copied, so "owned" is effectively free.
                    ParamOwnership::Owned
                } else {
                    ParamOwnership::Owned
                }
            }
        }
    }

    /// Register a new variable binding with its type.
    pub fn declare_variable(&mut self, name: String, span: Span, ty: LltsType) {
        let ownership = Self::ownership_for_type(&ty);
        self.variables.insert(
            name.clone(),
            OwnershipInfo {
                name,
                span,
                ty,
                ownership,
                moved: false,
            },
        );
    }

    /// Mark a variable as moved (e.g. passed to a function that takes ownership).
    pub fn mark_moved(&mut self, name: &str, _span: Span) {
        if let Some(info) = self.variables.get_mut(name) {
            if info.ownership == Ownership::Stack {
                // Stack values are copied, not moved.
                return;
            }
            info.moved = true;
            info.ownership = Ownership::Moved;
        }
    }

    /// Check if a variable can be used (not moved).
    pub fn check_use(&mut self, name: &str, span: Span) {
        if let Some(info) = self.variables.get(name) {
            if info.moved {
                self.errors.push(OwnershipError {
                    span,
                    kind: OwnershipErrorKind::UseAfterMove {
                        name: name.to_string(),
                    },
                });
            }
        }
    }

    /// Analyze a function body for ownership.
    pub fn analyze_function(
        &mut self,
        func: &Function<'_>,
        resolve_type: &dyn Fn(&TSType<'_>) -> LltsType,
    ) -> FunctionOwnership {
        let func_name = func
            .id
            .as_ref()
            .map(|id| id.name.to_string())
            .unwrap_or_else(|| "<anonymous>".to_string());

        // Analyze parameters
        let mut param_ownerships = Vec::new();
        for param in &func.params.items {
            let name = binding_pattern_name(&param.pattern);
            let ty = param
                .type_annotation
                .as_ref()
                .map(|ann| resolve_type(&ann.type_annotation))
                .unwrap_or(LltsType::Unknown);

            // For v1: determine usage by simple body scan
            let usage = self.scan_param_usage(&name, func.body.as_deref());
            let ownership = Self::infer_param_ownership(&ty, usage);

            self.declare_variable(name.clone(), param.span, ty);
            param_ownerships.push((name, ownership));
        }

        // Walk the body to track variable ownership
        if let Some(body) = &func.body {
            for stmt in &body.statements {
                self.analyze_statement(stmt, resolve_type);
            }
        }

        let locals: Vec<OwnershipInfo> = self.variables.values().cloned().collect();

        FunctionOwnership {
            name: func_name,
            params: param_ownerships,
            locals,
        }
    }

    /// Simple scan to determine how a parameter is used in a function body.
    fn scan_param_usage(&self, name: &str, body: Option<&FunctionBody<'_>>) -> Usage {
        let Some(body) = body else {
            return Usage::Read;
        };
        let mut usage = Usage::Read;
        for stmt in &body.statements {
            let stmt_usage = self.scan_statement_for_usage(name, stmt);
            usage = max_usage(usage, stmt_usage);
        }
        usage
    }

    fn scan_statement_for_usage(&self, name: &str, stmt: &Statement<'_>) -> Usage {
        match stmt {
            Statement::ExpressionStatement(expr_stmt) => {
                self.scan_expression_for_usage(name, &expr_stmt.expression)
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    let usage = self.scan_expression_for_usage(name, arg);
                    // If a variable is returned, it escapes
                    if usage != Usage::Read {
                        return usage;
                    }
                    if expr_references_name(arg, name) {
                        return Usage::Escape;
                    }
                }
                Usage::Read
            }
            Statement::BlockStatement(block) => {
                let mut usage = Usage::Read;
                for s in &block.body {
                    usage = max_usage(usage, self.scan_statement_for_usage(name, s));
                }
                usage
            }
            Statement::IfStatement(if_stmt) => {
                let mut usage = self.scan_statement_for_usage(name, &if_stmt.consequent);
                if let Some(alt) = &if_stmt.alternate {
                    usage = max_usage(usage, self.scan_statement_for_usage(name, alt));
                }
                usage
            }
            _ => Usage::Read,
        }
    }

    fn scan_expression_for_usage(&self, name: &str, expr: &Expression<'_>) -> Usage {
        match expr {
            // Method calls like `name.push(...)` indicate mutation
            Expression::CallExpression(call) => {
                if let Expression::StaticMemberExpression(member) = &call.callee {
                    if expr_references_name(&member.object, name) {
                        let method = member.property.name.as_str();
                        if is_mutating_method(method) {
                            return Usage::Mutate;
                        }
                    }
                }
                // If the variable is passed as an argument, check if it escapes
                for arg in &call.arguments {
                    match arg {
                        Argument::SpreadElement(spread) => {
                            if expr_references_name(&spread.argument, name) {
                                return Usage::Escape;
                            }
                        }
                        _ => {
                            if expr_references_name(arg.to_expression(), name) {
                                return Usage::Escape;
                            }
                        }
                    }
                }
                Usage::Read
            }
            // Assignment to a property of name: mutation
            Expression::AssignmentExpression(assign) => {
                if assignment_target_references_name(&assign.left, name) {
                    return Usage::Mutate;
                }
                // If the variable appears on the RHS stored somewhere, it escapes
                if expr_references_name(&assign.right, name) {
                    return Usage::Escape;
                }
                Usage::Read
            }
            _ => Usage::Read,
        }
    }

    fn analyze_statement(
        &mut self,
        stmt: &Statement<'_>,
        resolve_type: &dyn Fn(&TSType<'_>) -> LltsType,
    ) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    let name = binding_pattern_name(&declarator.id);
                    let ty = declarator
                        .type_annotation
                        .as_ref()
                        .map(|ann| resolve_type(&ann.type_annotation))
                        .or_else(|| {
                            declarator.init.as_ref().map(|_| LltsType::Unknown)
                        })
                        .unwrap_or(LltsType::Unknown);

                    self.declare_variable(name, declarator.span, ty);
                }
            }
            Statement::ExpressionStatement(expr_stmt) => {
                self.analyze_expression(&expr_stmt.expression);
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.analyze_expression(arg);
                }
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.analyze_statement(s, resolve_type);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.analyze_expression(&if_stmt.test);
                self.analyze_statement(&if_stmt.consequent, resolve_type);
                if let Some(alt) = &if_stmt.alternate {
                    self.analyze_statement(alt, resolve_type);
                }
            }
            _ => {}
        }
    }

    fn analyze_expression(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::Identifier(ident) => {
                self.check_use(ident.name.as_str(), ident.span);
            }
            Expression::CallExpression(call) => {
                self.analyze_expression(&call.callee);
                for arg in &call.arguments {
                    match arg {
                        Argument::SpreadElement(spread) => {
                            self.analyze_expression(&spread.argument);
                        }
                        _ => {
                            self.analyze_expression(arg.to_expression());
                        }
                    }
                }
            }
            Expression::AssignmentExpression(assign) => {
                self.analyze_expression(&assign.right);
            }
            _ => {}
        }
    }

    /// Consume the analyzer and return any ownership errors found.
    pub fn finish(self) -> Vec<OwnershipError> {
        self.errors
    }
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

fn expr_references_name(expr: &Expression<'_>, name: &str) -> bool {
    match expr {
        Expression::Identifier(ident) => ident.name.as_str() == name,
        Expression::StaticMemberExpression(member) => expr_references_name(&member.object, name),
        Expression::ComputedMemberExpression(member) => {
            expr_references_name(&member.object, name)
        }
        Expression::ParenthesizedExpression(paren) => {
            expr_references_name(&paren.expression, name)
        }
        _ => false,
    }
}

fn assignment_target_references_name(target: &AssignmentTarget<'_>, name: &str) -> bool {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            expr_references_name(&member.object, name)
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            expr_references_name(&member.object, name)
        }
        _ => false,
    }
}

/// Methods that mutate their receiver.
fn is_mutating_method(method: &str) -> bool {
    matches!(
        method,
        "push"
            | "pop"
            | "shift"
            | "unshift"
            | "splice"
            | "sort"
            | "reverse"
            | "fill"
            | "copyWithin"
            | "set"
            | "delete"
            | "clear"
    )
}

fn max_usage(a: Usage, b: Usage) -> Usage {
    match (a, b) {
        (Usage::Escape, _) | (_, Usage::Escape) => Usage::Escape,
        (Usage::Mutate, _) | (_, Usage::Mutate) => Usage::Mutate,
        _ => Usage::Read,
    }
}
