use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::{ParseOptions, Parser, ParserReturn};
use oxc_span::SourceType;

/// Result of parsing a TypeScript source file.
///
/// The caller must keep the `Allocator` alive for as long as the `Program` is used,
/// since the AST borrows from it. This struct packages the parser outputs so
/// downstream passes can inspect errors and access the AST.
pub struct ParseResult<'a> {
    /// The parsed AST program.
    pub program: Program<'a>,
    /// Syntax errors encountered during parsing.
    pub errors: Vec<OxcDiagnostic>,
    /// Whether the parser panicked and terminated early.
    pub panicked: bool,
}

impl<'a> ParseResult<'a> {
    /// Returns `true` if parsing succeeded without errors.
    pub fn is_ok(&self) -> bool {
        !self.panicked && self.errors.is_empty()
    }
}

/// Parse TypeScript source text into an oxc AST.
///
/// The `allocator` owns the memory backing the returned AST and must outlive
/// the returned `ParseResult`. `path` is used to infer `SourceType`
/// (TypeScript vs JavaScript, module vs script, JSX, etc.).
///
/// # Errors
///
/// Returns a `ParseResult` whose `errors` field is non-empty when the source
/// contains syntax errors. If the parser cannot recover, `panicked` will be
/// `true` and `program` will be empty.
pub fn parse_source<'a>(
    allocator: &'a Allocator,
    source_text: &'a str,
    path: &Path,
) -> ParseResult<'a> {
    let source_type = SourceType::from_path(path).unwrap_or_default();

    let ParserReturn {
        program,
        errors,
        panicked,
        ..
    } = Parser::new(allocator, source_text, source_type)
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();

    ParseResult {
        program,
        errors,
        panicked,
    }
}
