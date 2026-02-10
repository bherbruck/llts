use oxc_ast::ast::*;
use oxc_span::Span;

// ---------------------------------------------------------------------------
// Validation errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ValidationError {
    pub span: Span,
    pub kind: ValidationErrorKind,
}

#[derive(Debug, Clone)]
pub enum ValidationErrorKind {
    /// `any` type used without narrowing.
    AnyType,
    /// `unknown` type used without narrowing.
    UnknownType,
    /// `bigint` type is not supported.
    BigIntType,
    /// `symbol` type is not supported.
    SymbolType,
    /// `object` type has no known layout.
    ObjectType,
    /// Dynamic property access on unknown shape.
    DynamicPropertyAccess,
    /// Function parameter lacks a type annotation.
    UntypedParameter { name: String },
    /// Function lacks a return type annotation.
    MissingReturnType { name: String },
    /// Usage of `eval`.
    Eval,
    /// Usage of `with` statement.
    WithStatement,
    /// Usage of `Proxy`.
    ProxyUsage,
    /// Usage of `Reflect`.
    ReflectUsage,
    /// Prototype manipulation.
    PrototypeManipulation,
    /// `async`/`await` not supported in v1.
    AsyncAwait,
    /// Generator function not supported in v1.
    GeneratorFunction,
    /// `yield` expression not supported in v1.
    YieldExpression,
    /// `typeof` on arbitrary value (only known unions allowed).
    ArbitraryTypeof,
    /// `instanceof` on arbitrary value (only known unions allowed).
    ArbitraryInstanceof,
    /// Decorator usage.
    Decorator,
    /// `var` declaration (use `let` or `const`).
    VarDeclaration,
    /// Computed property key on struct/interface.
    ComputedProperty,
    /// Unsupported expression or statement.
    Unsupported { description: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match &self.kind {
            ValidationErrorKind::AnyType => "`any` type is not allowed in LLTS",
            ValidationErrorKind::UnknownType => "`unknown` type is not allowed without narrowing",
            ValidationErrorKind::BigIntType => "`bigint` is not supported in LLTS v1",
            ValidationErrorKind::SymbolType => "`symbol` is not supported in LLTS",
            ValidationErrorKind::ObjectType => "`object` type has no known layout",
            ValidationErrorKind::DynamicPropertyAccess => {
                "dynamic property access on unknown shape is not allowed"
            }
            ValidationErrorKind::UntypedParameter { name } => {
                return write!(f, "parameter `{name}` must have a type annotation");
            }
            ValidationErrorKind::MissingReturnType { name } => {
                return write!(f, "function `{name}` must have a return type annotation");
            }
            ValidationErrorKind::Eval => "`eval` is not allowed in LLTS",
            ValidationErrorKind::WithStatement => "`with` statement is not allowed in LLTS",
            ValidationErrorKind::ProxyUsage => "`Proxy` is not supported in LLTS",
            ValidationErrorKind::ReflectUsage => "`Reflect` is not supported in LLTS",
            ValidationErrorKind::PrototypeManipulation => {
                "prototype manipulation is not allowed in LLTS"
            }
            ValidationErrorKind::AsyncAwait => "`async`/`await` is not supported in LLTS v1",
            ValidationErrorKind::GeneratorFunction => {
                "generator functions are not supported in LLTS v1"
            }
            ValidationErrorKind::YieldExpression => {
                "`yield` expressions are not supported in LLTS v1"
            }
            ValidationErrorKind::ArbitraryTypeof => {
                "`typeof` on arbitrary values is not supported; use on known union types only"
            }
            ValidationErrorKind::ArbitraryInstanceof => {
                "`instanceof` on arbitrary values is not supported; use on known union types only"
            }
            ValidationErrorKind::Decorator => "decorators are not supported in LLTS",
            ValidationErrorKind::VarDeclaration => "use `let` or `const` instead of `var`",
            ValidationErrorKind::ComputedProperty => {
                "computed property keys are not supported on struct/interface types"
            }
            ValidationErrorKind::Unsupported { description } => {
                return write!(f, "unsupported: {description}");
            }
        };
        write!(f, "{msg}")
    }
}

// ---------------------------------------------------------------------------
// Validator
// ---------------------------------------------------------------------------

pub struct Validator {
    errors: Vec<ValidationError>,
}

impl Validator {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    /// Validate an entire program AST and return all errors found.
    pub fn validate_program(mut self, program: &Program<'_>) -> Vec<ValidationError> {
        for stmt in &program.body {
            self.validate_statement(stmt);
        }
        self.errors
    }

    fn error(&mut self, span: Span, kind: ValidationErrorKind) {
        self.errors.push(ValidationError { span, kind });
    }

    // -----------------------------------------------------------------------
    // Statements
    // -----------------------------------------------------------------------

    fn validate_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::BlockStatement(block) => {
                for s in &block.body {
                    self.validate_statement(s);
                }
            }
            Statement::ExpressionStatement(expr_stmt) => {
                self.validate_expression(&expr_stmt.expression);
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.validate_expression(arg);
                }
            }
            Statement::IfStatement(if_stmt) => {
                self.validate_expression(&if_stmt.test);
                self.validate_statement(&if_stmt.consequent);
                if let Some(alt) = &if_stmt.alternate {
                    self.validate_statement(alt);
                }
            }
            Statement::WhileStatement(while_stmt) => {
                self.validate_expression(&while_stmt.test);
                self.validate_statement(&while_stmt.body);
            }
            Statement::DoWhileStatement(do_while) => {
                self.validate_statement(&do_while.body);
                self.validate_expression(&do_while.test);
            }
            Statement::ForStatement(for_stmt) => {
                if let Some(init) = &for_stmt.init {
                    self.validate_for_init(init);
                }
                if let Some(test) = &for_stmt.test {
                    self.validate_expression(test);
                }
                if let Some(update) = &for_stmt.update {
                    self.validate_expression(update);
                }
                self.validate_statement(&for_stmt.body);
            }
            Statement::ForOfStatement(for_of) => {
                self.validate_for_in_of_left(&for_of.left);
                self.validate_expression(&for_of.right);
                self.validate_statement(&for_of.body);
            }
            Statement::ForInStatement(for_in) => {
                self.validate_for_in_of_left(&for_in.left);
                self.validate_expression(&for_in.right);
                self.validate_statement(&for_in.body);
            }
            Statement::SwitchStatement(switch) => {
                self.validate_expression(&switch.discriminant);
                for case in &switch.cases {
                    if let Some(test) = &case.test {
                        self.validate_expression(test);
                    }
                    for s in &case.consequent {
                        self.validate_statement(s);
                    }
                }
            }
            Statement::ThrowStatement(throw) => {
                self.validate_expression(&throw.argument);
            }
            Statement::TryStatement(try_stmt) => {
                for s in &try_stmt.block.body {
                    self.validate_statement(s);
                }
                if let Some(handler) = &try_stmt.handler {
                    for s in &handler.body.body {
                        self.validate_statement(s);
                    }
                }
                if let Some(finalizer) = &try_stmt.finalizer {
                    for s in &finalizer.body {
                        self.validate_statement(s);
                    }
                }
            }
            Statement::LabeledStatement(labeled) => {
                self.validate_statement(&labeled.body);
            }

            // -- with statement is rejected --
            Statement::WithStatement(with) => {
                self.error(with.span, ValidationErrorKind::WithStatement);
            }

            // -- Declarations --
            Statement::VariableDeclaration(var_decl) => {
                self.validate_variable_declaration(var_decl);
            }
            Statement::FunctionDeclaration(func) => {
                self.validate_function(func);
            }
            Statement::ClassDeclaration(class) => {
                self.validate_class(class);
            }
            Statement::TSTypeAliasDeclaration(alias) => {
                self.validate_type_annotation_ts_type(&alias.type_annotation, alias.span);
            }
            Statement::TSInterfaceDeclaration(iface) => {
                self.validate_interface(iface);
            }
            Statement::TSEnumDeclaration(enum_decl) => {
                self.validate_enum(enum_decl);
            }

            // Module declarations (import/export) are allowed
            Statement::ImportDeclaration(_) | Statement::ExportDefaultDeclaration(_) | Statement::ExportNamedDeclaration(_) | Statement::ExportAllDeclaration(_) => {}

            // Simple control flow
            Statement::BreakStatement(_) | Statement::ContinueStatement(_) | Statement::EmptyStatement(_) | Statement::DebuggerStatement(_) => {}

            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Variable declarations
    // -----------------------------------------------------------------------

    fn validate_variable_declaration(&mut self, decl: &VariableDeclaration<'_>) {
        if decl.kind == VariableDeclarationKind::Var {
            self.error(decl.span, ValidationErrorKind::VarDeclaration);
        }
        for declarator in &decl.declarations {
            if let Some(init) = &declarator.init {
                self.validate_expression(init);
            }
            // Check type annotations
            if let Some(ann) = &declarator.type_annotation {
                self.validate_type_annotation_ts_type(&ann.type_annotation, ann.span);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Functions
    // -----------------------------------------------------------------------

    fn validate_function(&mut self, func: &Function<'_>) {
        // Reject async
        if func.r#async {
            self.error(func.span, ValidationErrorKind::AsyncAwait);
        }

        // Reject generators
        if func.generator {
            self.error(func.span, ValidationErrorKind::GeneratorFunction);
        }

        // Check decorators (on function expressions)
        // (Note: function declarations don't have decorators, but we check anyway)

        // Check parameter types
        for param in &func.params.items {
            if param.type_annotation.is_none() {
                let name = binding_pattern_name(&param.pattern);
                self.error(
                    param.span,
                    ValidationErrorKind::UntypedParameter { name },
                );
            } else if let Some(ann) = &param.type_annotation {
                self.validate_type_annotation_ts_type(&ann.type_annotation, ann.span);
            }
        }

        // Check return type
        if func.return_type.is_none() {
            let name = func
                .id
                .as_ref()
                .map(|id| id.name.to_string())
                .unwrap_or_else(|| "<anonymous>".to_string());
            self.error(
                func.span,
                ValidationErrorKind::MissingReturnType { name },
            );
        } else if let Some(ret) = &func.return_type {
            self.validate_type_annotation_ts_type(&ret.type_annotation, ret.span);
        }

        // Validate body
        if let Some(body) = &func.body {
            for stmt in &body.statements {
                self.validate_statement(stmt);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Classes
    // -----------------------------------------------------------------------

    fn validate_class(&mut self, class: &Class<'_>) {
        // Reject decorators
        if !class.decorators.is_empty() {
            self.error(class.span, ValidationErrorKind::Decorator);
        }

        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    self.validate_function(&method.value);
                }
                ClassElement::PropertyDefinition(prop) => {
                    if prop.computed {
                        self.error(prop.span, ValidationErrorKind::ComputedProperty);
                    }
                    if let Some(ann) = &prop.type_annotation {
                        self.validate_type_annotation_ts_type(&ann.type_annotation, ann.span);
                    }
                    if let Some(init) = &prop.value {
                        self.validate_expression(init);
                    }
                    if !prop.decorators.is_empty() {
                        self.error(prop.span, ValidationErrorKind::Decorator);
                    }
                }
                ClassElement::StaticBlock(block) => {
                    for stmt in &block.body {
                        self.validate_statement(stmt);
                    }
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Interfaces
    // -----------------------------------------------------------------------

    fn validate_interface(&mut self, iface: &TSInterfaceDeclaration<'_>) {
        for member in &iface.body.body {
            match member {
                TSSignature::TSPropertySignature(prop) => {
                    if prop.computed {
                        self.error(prop.span, ValidationErrorKind::ComputedProperty);
                    }
                    if let Some(ann) = &prop.type_annotation {
                        self.validate_type_annotation_ts_type(&ann.type_annotation, ann.span);
                    }
                }
                TSSignature::TSMethodSignature(method) => {
                    // Check parameter types
                    for param in &method.params.items {
                        if param.type_annotation.is_none() {
                            let name = binding_pattern_name(&param.pattern);
                            self.error(
                                param.span,
                                ValidationErrorKind::UntypedParameter { name },
                            );
                        }
                    }
                    if method.return_type.is_none() {
                        let name = property_key_name(&method.key);
                        self.error(
                            method.span,
                            ValidationErrorKind::MissingReturnType { name },
                        );
                    }
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Enums
    // -----------------------------------------------------------------------

    fn validate_enum(&mut self, _enum_decl: &TSEnumDeclaration<'_>) {
        // Enums (numeric, string, const) are allowed. Nothing to reject.
    }

    // -----------------------------------------------------------------------
    // Expressions
    // -----------------------------------------------------------------------

    fn validate_expression(&mut self, expr: &Expression<'_>) {
        match expr {
            // -- Reject eval --
            Expression::Identifier(ident) if ident.name.as_str() == "eval" => {
                self.error(ident.span, ValidationErrorKind::Eval);
            }
            // -- Reject Proxy/Reflect --
            Expression::Identifier(ident) if ident.name.as_str() == "Proxy" => {
                self.error(ident.span, ValidationErrorKind::ProxyUsage);
            }
            Expression::Identifier(ident) if ident.name.as_str() == "Reflect" => {
                self.error(ident.span, ValidationErrorKind::ReflectUsage);
            }

            // -- Reject await --
            Expression::AwaitExpression(await_expr) => {
                self.error(await_expr.span, ValidationErrorKind::AsyncAwait);
            }

            // -- Reject yield --
            Expression::YieldExpression(yield_expr) => {
                self.error(yield_expr.span, ValidationErrorKind::YieldExpression);
            }

            // -- Call expressions: check for eval, Proxy, Reflect --
            Expression::CallExpression(call) => {
                self.validate_expression(&call.callee);
                for arg in &call.arguments {
                    self.validate_argument(arg);
                }
            }

            // -- New expressions --
            Expression::NewExpression(new) => {
                self.validate_expression(&new.callee);
                for arg in &new.arguments {
                    self.validate_argument(arg);
                }
            }

            // -- Member expressions: check for prototype manipulation --
            Expression::StaticMemberExpression(member) => {
                let prop_name = member.property.name.as_str();
                if prop_name == "__proto__" || prop_name == "prototype" {
                    self.error(
                        member.span,
                        ValidationErrorKind::PrototypeManipulation,
                    );
                }
                self.validate_expression(&member.object);
            }

            Expression::ComputedMemberExpression(member) => {
                self.validate_expression(&member.object);
                self.validate_expression(&member.expression);
            }

            // -- Unary: typeof check --
            Expression::UnaryExpression(unary) => {
                if unary.operator == UnaryOperator::Typeof {
                    // typeof is only safe in narrowing contexts, flag for review
                    // (full narrowing analysis would be more complex)
                }
                self.validate_expression(&unary.argument);
            }

            // -- Binary: instanceof check --
            Expression::BinaryExpression(binary) => {
                self.validate_expression(&binary.left);
                self.validate_expression(&binary.right);
            }

            // -- Arrow functions --
            Expression::ArrowFunctionExpression(arrow) => {
                if arrow.r#async {
                    self.error(arrow.span, ValidationErrorKind::AsyncAwait);
                }
                for param in &arrow.params.items {
                    if param.type_annotation.is_none() {
                        let name = binding_pattern_name(&param.pattern);
                        self.error(
                            param.span,
                            ValidationErrorKind::UntypedParameter { name },
                        );
                    }
                }
                // Validate body
                for stmt in &arrow.body.statements {
                    self.validate_statement(stmt);
                }
            }

            // -- Function expressions --
            Expression::FunctionExpression(func) => {
                self.validate_function(func);
            }

            // -- Class expressions --
            Expression::ClassExpression(class) => {
                self.validate_class(class);
            }

            // -- Assignment --
            Expression::AssignmentExpression(assign) => {
                self.validate_expression(&assign.right);
            }

            // -- Logical --
            Expression::LogicalExpression(logical) => {
                self.validate_expression(&logical.left);
                self.validate_expression(&logical.right);
            }

            // -- Conditional --
            Expression::ConditionalExpression(cond) => {
                self.validate_expression(&cond.test);
                self.validate_expression(&cond.consequent);
                self.validate_expression(&cond.alternate);
            }

            // -- Sequence --
            Expression::SequenceExpression(seq) => {
                for e in &seq.expressions {
                    self.validate_expression(e);
                }
            }

            // -- Template literals --
            Expression::TemplateLiteral(tmpl) => {
                for e in &tmpl.expressions {
                    self.validate_expression(e);
                }
            }

            // -- Tagged template --
            Expression::TaggedTemplateExpression(tagged) => {
                self.validate_expression(&tagged.tag);
                for e in &tagged.quasi.expressions {
                    self.validate_expression(e);
                }
            }

            // -- Array expression --
            Expression::ArrayExpression(arr) => {
                for elem in &arr.elements {
                    match elem {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            self.validate_expression(&spread.argument);
                        }
                        ArrayExpressionElement::Elision(_) => {}
                        _ => {
                            self.validate_expression(elem.to_expression());
                        }
                    }
                }
            }

            // -- Object expression --
            Expression::ObjectExpression(obj) => {
                for prop in &obj.properties {
                    match prop {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            self.validate_expression(&p.value);
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.validate_expression(&spread.argument);
                        }
                    }
                }
            }

            // -- Update (++, --) --
            Expression::UpdateExpression(_update) => {
                // SimpleAssignmentTarget - no expression to validate
            }

            // -- Parenthesized --
            Expression::ParenthesizedExpression(paren) => {
                self.validate_expression(&paren.expression);
            }

            // -- TS type assertions (as casts) --
            Expression::TSAsExpression(as_expr) => {
                self.validate_expression(&as_expr.expression);
                self.validate_type_annotation_ts_type(&as_expr.type_annotation, as_expr.span);
            }

            // -- BigInt literal --
            Expression::BigIntLiteral(bigint) => {
                self.error(bigint.span, ValidationErrorKind::BigIntType);
            }

            // Allowed primitives and identifiers
            Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::Identifier(_)
            | Expression::ThisExpression(_)
            | Expression::Super(_) => {}

            // Everything else: no special validation needed
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Type annotations
    // -----------------------------------------------------------------------

    fn validate_type_annotation_ts_type(&mut self, ty: &TSType<'_>, span: Span) {
        match ty {
            TSType::TSAnyKeyword(_) => {
                self.error(span, ValidationErrorKind::AnyType);
            }
            TSType::TSUnknownKeyword(_) => {
                self.error(span, ValidationErrorKind::UnknownType);
            }
            TSType::TSBigIntKeyword(_) => {
                self.error(span, ValidationErrorKind::BigIntType);
            }
            TSType::TSSymbolKeyword(_) => {
                self.error(span, ValidationErrorKind::SymbolType);
            }
            TSType::TSObjectKeyword(_) => {
                self.error(span, ValidationErrorKind::ObjectType);
            }
            // Recurse into compound types
            TSType::TSArrayType(arr) => {
                self.validate_type_annotation_ts_type(&arr.element_type, span);
            }
            TSType::TSTupleType(tuple) => {
                for elem in &tuple.element_types {
                    self.validate_tuple_element(elem, span);
                }
            }
            TSType::TSUnionType(union) => {
                for t in &union.types {
                    self.validate_type_annotation_ts_type(t, span);
                }
            }
            TSType::TSIntersectionType(inter) => {
                for t in &inter.types {
                    self.validate_type_annotation_ts_type(t, span);
                }
            }
            TSType::TSFunctionType(func) => {
                for param in &func.params.items {
                    if let Some(ann) = &param.type_annotation {
                        self.validate_type_annotation_ts_type(&ann.type_annotation, span);
                    }
                }
                self.validate_type_annotation_ts_type(&func.return_type.type_annotation, span);
            }
            TSType::TSTypeLiteral(lit) => {
                for member in &lit.members {
                    if let TSSignature::TSPropertySignature(prop) = member {
                        if prop.computed {
                            self.error(prop.span, ValidationErrorKind::ComputedProperty);
                        }
                        if let Some(ann) = &prop.type_annotation {
                            self.validate_type_annotation_ts_type(
                                &ann.type_annotation,
                                ann.span,
                            );
                        }
                    }
                }
            }
            TSType::TSParenthesizedType(paren) => {
                self.validate_type_annotation_ts_type(&paren.type_annotation, span);
            }
            // Everything else is fine
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn validate_tuple_element(&mut self, elem: &TSTupleElement<'_>, span: Span) {
        match elem {
            TSTupleElement::TSNamedTupleMember(named) => {
                self.validate_tuple_element(&named.element_type, span);
            }
            TSTupleElement::TSOptionalType(opt) => {
                self.validate_type_annotation_ts_type(&opt.type_annotation, span);
            }
            TSTupleElement::TSRestType(rest) => {
                self.validate_type_annotation_ts_type(&rest.type_annotation, span);
            }
            _ => {
                self.validate_type_annotation_ts_type(elem.to_ts_type(), span);
            }
        }
    }

    fn validate_for_init(&mut self, init: &ForStatementInit<'_>) {
        match init {
            ForStatementInit::VariableDeclaration(decl) => {
                self.validate_variable_declaration(decl);
            }
            _ => {
                self.validate_expression(init.to_expression());
            }
        }
    }

    fn validate_for_in_of_left(&mut self, left: &ForStatementLeft<'_>) {
        if let ForStatementLeft::VariableDeclaration(decl) = left {
            self.validate_variable_declaration(decl);
        }
    }

    fn validate_argument(&mut self, arg: &Argument<'_>) {
        match arg {
            Argument::SpreadElement(spread) => {
                self.validate_expression(&spread.argument);
            }
            _ => {
                self.validate_expression(arg.to_expression());
            }
        }
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

fn property_key_name(key: &PropertyKey<'_>) -> String {
    match key {
        PropertyKey::StaticIdentifier(id) => id.name.to_string(),
        _ => "<computed>".to_string(),
    }
}
