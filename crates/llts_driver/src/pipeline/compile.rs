use std::collections::HashSet;
use std::path::{Path, PathBuf};

use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};
use inkwell::passes::PassBuilderOptions;
use inkwell::OptimizationLevel;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;

use llts_codegen::{CodeGenerator, ProgramIR};
use llts_frontend::parse;
use llts_frontend::resolve::ModuleResolver;
use llts_frontend::semantic;

use super::context::LowerCtx;
use super::lower_program_with_ctx;

/// Compilation options.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Optimization level for LLVM passes.
    pub opt_level: OptimizationLevel,
    /// Whether to emit LLVM IR text instead of object code.
    pub emit_ir: bool,
    /// Output file path.
    pub output: String,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            opt_level: OptimizationLevel::Default,
            emit_ir: false,
            output: "a.out".to_string(),
        }
    }
}

/// Errors that can occur during compilation.
#[derive(Debug)]
pub enum CompileError {
    /// File I/O errors.
    Io(std::io::Error),
    /// Parse errors from the frontend.
    Parse(Vec<String>),
    /// Semantic analysis errors.
    Semantic(Vec<String>),
    /// Subset validation / type analysis errors.
    Analysis(Vec<String>),
    /// LLVM code generation or verification error.
    Codegen(String),
    /// Linker invocation failed.
    Link(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Io(e) => write!(f, "I/O error: {e}"),
            CompileError::Parse(errs) => {
                for e in errs {
                    writeln!(f, "parse error: {e}")?;
                }
                Ok(())
            }
            CompileError::Semantic(errs) => {
                for e in errs {
                    writeln!(f, "semantic error: {e}")?;
                }
                Ok(())
            }
            CompileError::Analysis(errs) => {
                for e in errs {
                    writeln!(f, "analysis error: {e}")?;
                }
                Ok(())
            }
            CompileError::Codegen(e) => write!(f, "codegen error: {e}"),
            CompileError::Link(e) => write!(f, "link error: {e}"),
        }
    }
}

/// Compile a TypeScript source file to a native binary.
///
/// This runs the full pipeline:
/// 1. Resolve module graph (walk imports recursively)
/// 2. For each file (dependencies first): parse → analyze → lower
/// 3. Merge all IR into a single ProgramIR
/// 4. LLVM IR generation (llts_codegen)
/// 5. LLVM optimization
/// 6. Object emission + linking
pub fn compile_file(path: &Path, options: &CompileOptions) -> Result<(), CompileError> {
    let abs_path = std::fs::canonicalize(path)
        .map_err(CompileError::Io)?;

    // Stage 1: Resolve module graph (entry + all transitive imports)
    let file_order = resolve_module_graph(&abs_path)?;

    // Stage 2-4: Parse, analyze, and lower each file with shared context
    let mut ctx = LowerCtx::new();
    let mut merged_ir = ProgramIR {
        structs: Vec::new(),
        enums: Vec::new(),
        functions: Vec::new(),
    };

    for file_path in &file_order {
        let is_entry = file_path == &abs_path;
        let ir = compile_single_file(file_path, &mut ctx, is_entry)?;
        merged_ir.structs.extend(ir.structs);
        merged_ir.enums.extend(ir.enums);
        merged_ir.functions.extend(ir.functions);
    }

    // Stage 5: LLVM IR generation
    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "main");
    codegen.compile(&merged_ir);

    // Verify the module
    if let Err(msg) = codegen.module().verify() {
        return Err(CompileError::Codegen(msg.to_string()));
    }

    if options.emit_ir {
        // Write LLVM IR text
        let ir = codegen.module().print_to_string().to_string();
        std::fs::write(&options.output, ir).map_err(CompileError::Io)?;
        return Ok(());
    }

    // Stage 6 & 7: Optimize + emit object file + link
    let module = codegen.into_module();
    emit_and_link(&module, options)
}

/// Parse, analyze, and lower a single file to ProgramIR.
/// Uses the shared `LowerCtx` so imported types/functions are available.
/// If `is_entry` is false, the `main` function is excluded.
pub(crate) fn compile_single_file(
    path: &Path,
    ctx: &mut LowerCtx,
    is_entry: bool,
) -> Result<ProgramIR, CompileError> {
    let source_text = std::fs::read_to_string(path).map_err(CompileError::Io)?;

    // Parse
    let allocator = Allocator::default();
    let parse_result = parse::parse_source(&allocator, &source_text, path);
    if !parse_result.is_ok() {
        return Err(CompileError::Parse(
            parse_result
                .errors
                .iter()
                .map(|e| format!("{}: {e}", path.display()))
                .collect(),
        ));
    }

    // Semantic analysis
    let sem_result = semantic::analyze_semantics(&parse_result.program);
    if !sem_result.is_ok() {
        return Err(CompileError::Semantic(
            sem_result
                .errors
                .iter()
                .map(|e| format!("{}: {e}", path.display()))
                .collect(),
        ));
    }

    // Subset validation + type resolution
    let analysis_result = llts_analysis::analyze(&parse_result.program);
    if analysis_result.has_errors() {
        return Err(CompileError::Analysis(
            analysis_result
                .errors
                .iter()
                .map(|e| format!("{}: {e}", path.display()))
                .collect(),
        ));
    }

    // Lower AST → codegen IR with shared context
    let ir = lower_program_with_ctx(&parse_result.program, ctx, is_entry);
    Ok(ir)
}

/// Resolve the module graph starting from an entry file.
/// Returns files in dependency order (imports first, entry last).
pub(crate) fn resolve_module_graph(entry: &Path) -> Result<Vec<PathBuf>, CompileError> {
    let resolver = ModuleResolver::new();
    let mut visited = HashSet::new();
    let mut order = Vec::new();

    fn walk(
        file: &Path,
        resolver: &ModuleResolver,
        visited: &mut HashSet<PathBuf>,
        order: &mut Vec<PathBuf>,
    ) -> Result<(), CompileError> {
        if visited.contains(file) {
            return Ok(()); // Already processed (handles circular imports)
        }
        visited.insert(file.to_path_buf());

        // Parse just to extract imports (lightweight — no analysis)
        let source = std::fs::read_to_string(file).map_err(CompileError::Io)?;
        let allocator = Allocator::default();
        let parse_result = parse::parse_source(&allocator, &source, file);
        if !parse_result.is_ok() {
            return Err(CompileError::Parse(
                parse_result
                    .errors
                    .iter()
                    .map(|e| format!("{}: {e}", file.display()))
                    .collect(),
            ));
        }

        // Extract import specifiers and resolve them
        for stmt in &parse_result.program.body {
            if let Statement::ImportDeclaration(import) = stmt {
                let specifier = import.source.value.as_str();
                let resolved = resolver
                    .resolve_from_file(file, specifier)
                    .map_err(|e| {
                        CompileError::Analysis(vec![format!(
                            "{}: cannot resolve import '{}': {e}",
                            file.display(),
                            specifier
                        )])
                    })?;
                let import_path = resolved.into_path_buf();
                walk(&import_path, resolver, visited, order)?;
            }
        }

        // Add this file after its dependencies
        order.push(file.to_path_buf());
        Ok(())
    }

    walk(entry, &resolver, &mut visited, &mut order)?;
    Ok(order)
}

/// Emit the LLVM module to an object file and link it to produce a binary.
pub(crate) fn emit_and_link(module: &Module<'_>, options: &CompileOptions) -> Result<(), CompileError> {
    Target::initialize_native(&InitializationConfig::default())
        .map_err(|e| CompileError::Codegen(e.to_string()))?;

    let target_triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&target_triple)
        .map_err(|e| CompileError::Codegen(e.to_string()))?;

    let cpu = TargetMachine::get_host_cpu_name();
    let features = TargetMachine::get_host_cpu_features();

    let machine = target
        .create_target_machine(
            &target_triple,
            cpu.to_str().unwrap_or("generic"),
            features.to_str().unwrap_or(""),
            options.opt_level,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .ok_or_else(|| CompileError::Codegen("failed to create target machine".into()))?;

    // Run LLVM optimization passes on the module
    let pass = match options.opt_level {
        OptimizationLevel::None => None,
        OptimizationLevel::Less => Some("default<O1>"),
        OptimizationLevel::Default => Some("default<O2>"),
        OptimizationLevel::Aggressive => Some("default<O3>"),
    };

    if let Some(passes) = pass {
        module
            .run_passes(passes, &machine, PassBuilderOptions::create())
            .map_err(|e| CompileError::Codegen(e.to_string()))?;
    }

    let obj_path = format!("{}.o", options.output);
    machine
        .write_to_file(module, FileType::Object, Path::new(&obj_path))
        .map_err(|e| CompileError::Codegen(e.to_string()))?;

    // Link with system linker
    crate::linker::link(&obj_path, &options.output)?;

    // Clean up the intermediate object file
    let _ = std::fs::remove_file(&obj_path);

    Ok(())
}
