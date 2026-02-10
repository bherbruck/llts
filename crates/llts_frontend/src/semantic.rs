use oxc_ast::ast::Program;
use oxc_diagnostics::OxcDiagnostic;
use oxc_semantic::{Semantic, SemanticBuilder, SemanticBuilderReturn};

/// Result of semantic analysis on a parsed program.
pub struct SemanticResult<'a> {
    /// The semantic information (scopes, symbols, nodes, etc.).
    pub semantic: Semantic<'a>,
    /// Semantic errors (e.g. redeclarations, invalid syntax caught in this pass).
    pub errors: Vec<OxcDiagnostic>,
}

impl<'a> SemanticResult<'a> {
    /// Returns `true` if semantic analysis found no errors.
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Run semantic analysis on a parsed program.
///
/// `program` must be a reference into allocator-owned memory (i.e.
/// `&'a Program<'a>` where the allocator outlives both). This performs scope
/// and symbol resolution, reference binding, and optional syntax error
/// checking.
pub fn analyze_semantics<'a>(program: &'a Program<'a>) -> SemanticResult<'a> {
    let SemanticBuilderReturn { semantic, errors } = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(program);

    SemanticResult { semantic, errors }
}
