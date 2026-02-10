use std::collections::HashMap;

use oxc_ast::ast::*;
use oxc_span::Span;

use crate::types::LltsType;

// ---------------------------------------------------------------------------
// Borrow state
// ---------------------------------------------------------------------------

/// The borrow state of a variable at a program point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowState {
    /// Not currently borrowed.
    Unborrowed,
    /// Immutably borrowed (multiple readers allowed).
    ImmutableBorrow { count: u32 },
    /// Mutably borrowed (exclusive access).
    MutableBorrow,
    /// Value has been moved out; no access allowed.
    Moved,
}

/// A record of an active borrow.
#[derive(Debug, Clone)]
pub struct BorrowRecord {
    pub variable: String,
    pub borrow_kind: BorrowKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowKind {
    Immutable,
    Mutable,
}

// ---------------------------------------------------------------------------
// Borrow errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BorrowError {
    pub span: Span,
    pub kind: BorrowErrorKind,
}

#[derive(Debug, Clone)]
pub enum BorrowErrorKind {
    /// Attempt to mutably borrow a variable that has outstanding immutable borrows.
    MutableBorrowWhileImmutablelyBorrowed { name: String },
    /// Attempt to borrow (immutably) a variable that is mutably borrowed.
    ImmutableBorrowWhileMutablyBorrowed { name: String },
    /// Attempt to mutably borrow a variable that is already mutably borrowed.
    DoubleMutableBorrow { name: String },
    /// Attempt to mutate through a `Readonly<T>` reference.
    MutateReadonly { name: String },
    /// Use of a variable after it has been moved.
    UseAfterMove { name: String },
}

impl std::fmt::Display for BorrowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            BorrowErrorKind::MutableBorrowWhileImmutablelyBorrowed { name } => {
                write!(f, "cannot mutably borrow `{name}` while it is immutably borrowed")
            }
            BorrowErrorKind::ImmutableBorrowWhileMutablyBorrowed { name } => {
                write!(f, "cannot borrow `{name}` while it is mutably borrowed")
            }
            BorrowErrorKind::DoubleMutableBorrow { name } => {
                write!(f, "cannot mutably borrow `{name}` more than once at a time")
            }
            BorrowErrorKind::MutateReadonly { name } => {
                write!(f, "cannot mutate `{name}` through Readonly reference")
            }
            BorrowErrorKind::UseAfterMove { name } => {
                write!(f, "use of moved variable `{name}`")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Borrow checker
// ---------------------------------------------------------------------------

pub struct BorrowChecker {
    /// Variable -> borrow state.
    states: HashMap<String, BorrowState>,
    /// Variable -> whether the type is Readonly<T>.
    readonly: HashMap<String, bool>,
    /// Variable -> whether the type is Copy (primitives, enums without payload).
    copy_types: HashMap<String, bool>,
    errors: Vec<BorrowError>,
}

impl BorrowChecker {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            readonly: HashMap::new(),
            copy_types: HashMap::new(),
            errors: Vec::new(),
        }
    }

    /// Register a variable for tracking.
    pub fn declare(&mut self, name: String, ty: &LltsType) {
        self.states.insert(name.clone(), BorrowState::Unborrowed);
        let is_readonly = matches!(ty, LltsType::Readonly(_));
        self.readonly.insert(name.clone(), is_readonly);
        self.copy_types.insert(name, ty.is_copy());
    }

    /// Record an immutable borrow of a variable.
    pub fn borrow_immutable(&mut self, name: &str, span: Span) {
        match self.states.get(name).copied() {
            Some(BorrowState::Moved) => {
                self.errors.push(BorrowError {
                    span,
                    kind: BorrowErrorKind::UseAfterMove {
                        name: name.to_string(),
                    },
                });
            }
            Some(BorrowState::MutableBorrow) => {
                self.errors.push(BorrowError {
                    span,
                    kind: BorrowErrorKind::ImmutableBorrowWhileMutablyBorrowed {
                        name: name.to_string(),
                    },
                });
            }
            Some(BorrowState::ImmutableBorrow { count }) => {
                self.states.insert(
                    name.to_string(),
                    BorrowState::ImmutableBorrow { count: count + 1 },
                );
            }
            Some(BorrowState::Unborrowed) => {
                self.states.insert(
                    name.to_string(),
                    BorrowState::ImmutableBorrow { count: 1 },
                );
            }
            None => {
                // Variable not tracked (may be from outer scope)
            }
        }
    }

    /// Record a mutable borrow of a variable.
    pub fn borrow_mutable(&mut self, name: &str, span: Span) {
        // Check Readonly constraint
        if self.readonly.get(name).copied().unwrap_or(false) {
            self.errors.push(BorrowError {
                span,
                kind: BorrowErrorKind::MutateReadonly {
                    name: name.to_string(),
                },
            });
            return;
        }

        match self.states.get(name).copied() {
            Some(BorrowState::Moved) => {
                self.errors.push(BorrowError {
                    span,
                    kind: BorrowErrorKind::UseAfterMove {
                        name: name.to_string(),
                    },
                });
            }
            Some(BorrowState::MutableBorrow) => {
                self.errors.push(BorrowError {
                    span,
                    kind: BorrowErrorKind::DoubleMutableBorrow {
                        name: name.to_string(),
                    },
                });
            }
            Some(BorrowState::ImmutableBorrow { .. }) => {
                self.errors.push(BorrowError {
                    span,
                    kind: BorrowErrorKind::MutableBorrowWhileImmutablelyBorrowed {
                        name: name.to_string(),
                    },
                });
            }
            Some(BorrowState::Unborrowed) => {
                self.states
                    .insert(name.to_string(), BorrowState::MutableBorrow);
            }
            None => {}
        }
    }

    /// Release a borrow (e.g. at end of the borrowing scope).
    pub fn release_borrow(&mut self, name: &str) {
        match self.states.get(name).copied() {
            Some(BorrowState::ImmutableBorrow { count }) if count > 1 => {
                self.states.insert(
                    name.to_string(),
                    BorrowState::ImmutableBorrow { count: count - 1 },
                );
            }
            Some(BorrowState::ImmutableBorrow { .. }) | Some(BorrowState::MutableBorrow) => {
                self.states
                    .insert(name.to_string(), BorrowState::Unborrowed);
            }
            _ => {}
        }
    }

    /// Mark a variable as moved. Copy types are never moved.
    pub fn mark_moved(&mut self, name: &str, span: Span) {
        // Copy types are implicitly copied, never moved.
        if self.copy_types.get(name).copied().unwrap_or(false) {
            return;
        }
        if let Some(state) = self.states.get(name).copied() {
            if state == BorrowState::Moved {
                self.errors.push(BorrowError {
                    span,
                    kind: BorrowErrorKind::UseAfterMove {
                        name: name.to_string(),
                    },
                });
                return;
            }
        }
        self.states.insert(name.to_string(), BorrowState::Moved);
    }

    /// Check that a variable can be used (not moved).
    pub fn check_use(&mut self, name: &str, span: Span) {
        if let Some(BorrowState::Moved) = self.states.get(name) {
            self.errors.push(BorrowError {
                span,
                kind: BorrowErrorKind::UseAfterMove {
                    name: name.to_string(),
                },
            });
        }
    }

    /// Check that a variable can be mutated (not readonly, not immutably borrowed).
    pub fn check_mutation(&mut self, name: &str, span: Span) {
        if self.readonly.get(name).copied().unwrap_or(false) {
            self.errors.push(BorrowError {
                span,
                kind: BorrowErrorKind::MutateReadonly {
                    name: name.to_string(),
                },
            });
        }
    }

    /// Run a basic borrow check pass over a function body.
    pub fn check_function(&mut self, func: &Function<'_>, resolve_type: &dyn Fn(&TSType<'_>) -> LltsType) {
        // Register parameters
        for param in &func.params.items {
            let name = binding_pattern_name(&param.pattern);
            let ty = param
                .type_annotation
                .as_ref()
                .map(|ann| resolve_type(&ann.type_annotation))
                .unwrap_or(LltsType::Unknown);
            self.declare(name, &ty);
        }

        // Walk the body
        if let Some(body) = &func.body {
            for stmt in &body.statements {
                self.check_statement(stmt, resolve_type);
            }
        }
    }

    fn check_statement(&mut self, stmt: &Statement<'_>, resolve_type: &dyn Fn(&TSType<'_>) -> LltsType) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    let name = binding_pattern_name(&declarator.id);
                    let ty = declarator
                        .type_annotation
                        .as_ref()
                        .map(|ann| resolve_type(&ann.type_annotation))
                        .unwrap_or(LltsType::Unknown);
                    self.declare(name, &ty);
                    if let Some(init) = &declarator.init {
                        self.check_expression(init);
                    }
                }
            }
            Statement::ExpressionStatement(expr_stmt) => {
                self.check_expression(&expr_stmt.expression);
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    // Returning a variable moves it
                    if let Expression::Identifier(ident) = arg {
                        self.mark_moved(ident.name.as_str(), ident.span);
                    } else {
                        self.check_expression(arg);
                    }
                }
            }
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.check_statement(s, resolve_type);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.check_expression(&if_stmt.test);
                self.check_statement(&if_stmt.consequent, resolve_type);
                if let Some(alt) = &if_stmt.alternate {
                    self.check_statement(alt, resolve_type);
                }
            }
            _ => {}
        }
    }

    fn check_expression(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::Identifier(ident) => {
                self.check_use(ident.name.as_str(), ident.span);
            }
            Expression::AssignmentExpression(assign) => {
                // Check mutation on the target
                if let Some(name) = assignment_target_name(&assign.left) {
                    self.check_mutation(&name, assign.span);
                }
                self.check_expression(&assign.right);
            }
            Expression::CallExpression(call) => {
                // Check for mutating method calls
                if let Expression::StaticMemberExpression(member) = &call.callee {
                    let method = member.property.name.as_str();
                    if is_mutating_method(method) {
                        if let Expression::Identifier(ident) = &member.object {
                            self.check_mutation(ident.name.as_str(), call.span);
                            self.borrow_mutable(ident.name.as_str(), call.span);
                        }
                    } else {
                        if let Expression::Identifier(ident) = &member.object {
                            self.borrow_immutable(ident.name.as_str(), call.span);
                        }
                    }
                }
                for arg in &call.arguments {
                    match arg {
                        Argument::SpreadElement(spread) => {
                            self.check_expression(&spread.argument);
                        }
                        _ => {
                            self.check_expression(arg.to_expression());
                        }
                    }
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.check_expression(&member.object);
            }
            Expression::BinaryExpression(binary) => {
                self.check_expression(&binary.left);
                self.check_expression(&binary.right);
            }
            Expression::UnaryExpression(unary) => {
                self.check_expression(&unary.argument);
            }
            _ => {}
        }
    }

    /// Consume the checker and return all errors.
    pub fn finish(self) -> Vec<BorrowError> {
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

fn assignment_target_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(ident) => Some(ident.name.to_string()),
        AssignmentTarget::StaticMemberExpression(member) => {
            if let Expression::Identifier(ident) = &member.object {
                Some(ident.name.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

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
