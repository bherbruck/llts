use std::collections::{HashMap, HashSet};

use oxc_ast::ast::*;

use llts_codegen::{
    EnumDecl, Expr, FunctionDecl, ProgramIR, Stmt, StructDecl,
    types::LltsType,
};

mod compile;
mod context;
mod exprs;
mod generics;
mod stmts;
mod types;
mod unions;
mod utils;
pub use compile::{compile_file, CompileError, CompileOptions};
pub(crate) use context::*;
pub(crate) use exprs::*;
pub(crate) use generics::*;
pub(crate) use stmts::*;
pub(crate) use types::*;
pub(crate) use unions::*;
pub(crate) use utils::*;

// ---------------------------------------------------------------------------
// AST → ProgramIR lowering
// ---------------------------------------------------------------------------

/// Lower a program with a shared LowerCtx (for multi-file compilation).
/// If `is_entry` is false, `main` functions are excluded from the output.
pub(crate) fn lower_program_with_ctx(
    program: &Program<'_>,
    ctx: &mut LowerCtx,
    is_entry: bool,
) -> ProgramIR {
    let mut structs = Vec::new();
    let mut enums = Vec::new();
    let mut functions = Vec::new();
    let mut top_level_vars: Vec<Stmt> = Vec::new();

    // Helper: collect type declarations from a Declaration node (used for exports)
    fn collect_type_decl<'a>(
        decl: &'a Declaration<'a>,
        ctx: &mut LowerCtx,
        structs: &mut Vec<StructDecl>,
        enums: &mut Vec<EnumDecl>,
    ) {
        match decl {
            Declaration::TSInterfaceDeclaration(iface) => {
                if let Some(s) = lower_interface(iface) {
                    for (field_name, lit_value) in extract_string_literal_fields(iface) {
                        ctx.string_literal_fields.insert(
                            (s.name.clone(), field_name),
                            lit_value,
                        );
                    }
                    ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                    structs.push(s);
                }
            }
            Declaration::ClassDeclaration(class) => {
                if let Some(s) = lower_class_struct(class) {
                    ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                    structs.push(s);
                }
            }
            Declaration::TSEnumDeclaration(d) => {
                let (e, variant_values) = lower_enum(d);
                ctx.enum_defs.insert(e.name.clone(), variant_values);
                enums.push(e);
            }
            Declaration::TSTypeAliasDeclaration(alias) => {
                if let Some(s) = lower_type_alias(alias) {
                    ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                    structs.push(s);
                }
                detect_discriminated_union(alias, ctx, structs, enums);
            }
            _ => {}
        }
    }

    // Helper: collect function return type and parameter types from a Declaration node
    fn collect_fn_sig(decl: &Declaration<'_>, ctx: &mut LowerCtx, enum_names: &HashSet<String>) {
        if let Declaration::FunctionDeclaration(func) = decl {
            if let Some(id) = &func.id {
                let ret_type = func
                    .return_type
                    .as_ref()
                    .map(|r| lower_ts_type_with_enums(&r.type_annotation, enum_names))
                    .unwrap_or(LltsType::Void);
                let ret_type = match &ret_type {
                    LltsType::Struct { name, fields } if fields.is_empty() => {
                        if let Some(du) = ctx.discriminated_unions.get(name) {
                            du.union_type.clone()
                        } else {
                            ctx.full_struct_type(name)
                        }
                    }
                    other => other.clone(),
                };
                ctx.fn_ret_types.insert(id.name.to_string(), ret_type);
                // Collect parameter types
                let param_types: Vec<LltsType> = func.params.items.iter().map(|p| {
                    let pty = p.type_annotation.as_ref()
                        .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, enum_names))
                        .unwrap_or(LltsType::F64);
                    match &pty {
                        LltsType::Struct { name, fields } if fields.is_empty() => ctx.full_struct_type(name),
                        other => other.clone(),
                    }
                }).collect();
                ctx.fn_param_types.insert(id.name.to_string(), param_types);
            }
        }
    }

    // First pass: collect all struct/interface/enum definitions + register generic functions
    for (stmt_idx, stmt) in program.body.iter().enumerate() {
        match stmt {
            // Register generic function definitions (have TSTypeParameterDeclaration)
            Statement::FunctionDeclaration(func) => {
                if let Some(type_params) = &func.type_parameters {
                    if !type_params.params.is_empty() {
                        if let Some(id) = &func.id {
                            let name = id.name.to_string();
                            ctx.generic_fn_indices.insert(name.clone(), stmt_idx);
                            ctx.generic_fn_params.insert(name, extract_generic_param_info(type_params, &ctx.type_alias_members));
                        }
                    }
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(Declaration::FunctionDeclaration(func)) = &export.declaration {
                    if let Some(type_params) = &func.type_parameters {
                        if !type_params.params.is_empty() {
                            if let Some(id) = &func.id {
                                let name = id.name.to_string();
                                ctx.generic_fn_indices.insert(name.clone(), stmt_idx);
                                ctx.generic_fn_params.insert(name, extract_generic_param_info(type_params, &ctx.type_alias_members));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        // Also collect type declarations in this same loop
        match stmt {
            Statement::TSInterfaceDeclaration(iface) => {
                if let Some(s) = lower_interface(iface) {
                    // Extract string literal fields for discriminated union detection.
                    for (field_name, lit_value) in extract_string_literal_fields(iface) {
                        ctx.string_literal_fields.insert(
                            (s.name.clone(), field_name),
                            lit_value,
                        );
                    }
                    ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                    structs.push(s);
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(s) = lower_class_struct(class) {
                    ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                    structs.push(s);
                }
            }
            Statement::TSEnumDeclaration(decl) => {
                let (e, variant_values) = lower_enum(decl);
                ctx.enum_defs.insert(e.name.clone(), variant_values);
                enums.push(e);
            }
            Statement::TSTypeAliasDeclaration(alias) => {
                if let Some(s) = lower_type_alias(alias) {
                    ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                    structs.push(s);
                }
                // Store non-struct type aliases (unions, primitives, etc.)
                let alias_name = alias.id.name.to_string();
                if !ctx.struct_defs.contains_key(&alias_name) {
                    let resolved = lower_ts_type(&alias.type_annotation);
                    ctx.type_aliases.insert(alias_name.clone(), resolved);
                    // For union type aliases, also store the individual members (pre-widening)
                    if let TSType::TSUnionType(union) = &alias.type_annotation {
                        let members: Vec<LltsType> = union.types.iter().map(|t| lower_ts_type(t)).collect();
                        ctx.type_alias_members.insert(alias_name.clone(), members);
                    }
                }
                // Detect discriminated unions: type Shape = Circle | Rectangle
                detect_discriminated_union(alias, ctx, &mut structs, &mut enums);
            }
            // Unwrap export declarations
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    collect_type_decl(decl, ctx, &mut structs, &mut enums);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::ClassDeclaration(class) = &export.declaration {
                    if let Some(s) = lower_class_struct(class) {
                        ctx.struct_defs.insert(s.name.clone(), s.fields.clone());
                        structs.push(s);
                    }
                }
            }
            _ => {}
        }
    }

    // Second pass: collect function return types (enums are now registered)
    // Skip generic functions — their return types depend on type parameters.
    let enum_names = ctx.enum_names();
    for stmt in &program.body {
        match stmt {
            Statement::FunctionDeclaration(func) => {
                if let Some(id) = &func.id {
                    // Skip generic functions
                    if ctx.generic_fn_indices.contains_key(id.name.as_str()) {
                        continue;
                    }
                    let ret_type = func
                        .return_type
                        .as_ref()
                        .map(|r| lower_ts_type_with_enums(&r.type_annotation, &enum_names))
                        .unwrap_or(LltsType::Void);
                    let ret_type = match &ret_type {
                        LltsType::Struct { name, fields } if fields.is_empty() => {
                            if let Some(du) = ctx.discriminated_unions.get(name) {
                                du.union_type.clone()
                            } else {
                                ctx.full_struct_type(name)
                            }
                        }
                        other => other.clone(),
                    };
                    ctx.fn_ret_types.insert(id.name.to_string(), ret_type);
                    // Collect parameter types
                    let param_types: Vec<LltsType> = func.params.items.iter().map(|p| {
                        let pty = p.type_annotation.as_ref()
                            .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, &enum_names))
                            .unwrap_or(LltsType::F64);
                        match &pty {
                            LltsType::Struct { name, fields } if fields.is_empty() => ctx.full_struct_type(name),
                            other => other.clone(),
                        }
                    }).collect();
                    ctx.fn_param_types.insert(id.name.to_string(), param_types);
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    collect_fn_sig(decl, ctx, &enum_names);
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::FunctionDeclaration(func) = &export.declaration {
                    if let Some(id) = &func.id {
                        let ret_type = func
                            .return_type
                            .as_ref()
                            .map(|r| lower_ts_type_with_enums(&r.type_annotation, &enum_names))
                            .unwrap_or(LltsType::Void);
                        let ret_type = match &ret_type {
                            LltsType::Struct { name, fields } if fields.is_empty() => {
                                if let Some(du) = ctx.discriminated_unions.get(name) {
                                    du.union_type.clone()
                                } else {
                                    ctx.full_struct_type(name)
                                }
                            }
                            other => other.clone(),
                        };
                        ctx.fn_ret_types.insert(id.name.to_string(), ret_type);
                        // Collect parameter types
                        let param_types: Vec<LltsType> = func.params.items.iter().map(|p| {
                            let pty = p.type_annotation.as_ref()
                                .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, &enum_names))
                                .unwrap_or(LltsType::F64);
                            match &pty {
                                LltsType::Struct { name, fields } if fields.is_empty() => ctx.full_struct_type(name),
                                other => other.clone(),
                            }
                        }).collect();
                        ctx.fn_param_types.insert(id.name.to_string(), param_types);
                    }
                }
            }
            _ => {}
        }
    }

    // Third pass: lower functions and class methods with full context
    // Skip generic functions — they are monomorphized on-demand at call sites.
    for stmt in &program.body {
        match stmt {
            Statement::FunctionDeclaration(func) => {
                // Skip generic function definitions
                if let Some(id) = &func.id {
                    if ctx.generic_fn_indices.contains_key(id.name.as_str()) {
                        continue;
                    }
                }
                if !is_entry {
                    if let Some(id) = &func.id {
                        if id.name.as_str() == "main" {
                            continue;
                        }
                    }
                }
                if let Some(f) = lower_function(func, ctx) {
                    functions.push(f);
                }
            }
            Statement::ClassDeclaration(class) => {
                functions.extend(lower_class_methods(class, ctx));
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    match decl {
                        Declaration::FunctionDeclaration(func) => {
                            // Skip generic function definitions
                            if let Some(id) = &func.id {
                                if ctx.generic_fn_indices.contains_key(id.name.as_str()) {
                                    continue;
                                }
                            }
                            if !is_entry {
                                if let Some(id) = &func.id {
                                    if id.name.as_str() == "main" {
                                        continue;
                                    }
                                }
                            }
                            if let Some(f) = lower_function(func, ctx) {
                                functions.push(f);
                            }
                        }
                        Declaration::ClassDeclaration(class) => {
                            functions.extend(lower_class_methods(class, ctx));
                        }
                        Declaration::VariableDeclaration(var_decl) => {
                            for declarator in &var_decl.declarations {
                                let name = binding_name(&declarator.id);
                                let ty = declarator
                                    .type_annotation
                                    .as_ref()
                                    .map(|ann| lower_ts_type_with_enums(&ann.type_annotation, &ctx.enum_names()))
                                    .or_else(|| declarator.init.as_ref().map(|e| infer_expr_type(e)))
                                    .unwrap_or(LltsType::F64);
                                ctx.var_types.insert(name.clone(), ty.clone());
                                let init = declarator.init.as_ref().map(|e| lower_expr(e, ctx));
                                // Skip arrow function initializers (handled as pending functions)
                                if let Some(Expr::Var { name: ref ln, .. }) = init {
                                    if ln.starts_with("__lambda_") { continue; }
                                }
                                top_level_vars.push(Stmt::VarDecl { name, ty, init });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                match &export.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                        if let Some(f) = lower_function(func, ctx) {
                            functions.push(f);
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        functions.extend(lower_class_methods(class, ctx));
                    }
                    _ => {}
                }
            }
            // Top-level variable declarations: lower into statements for main
            Statement::VariableDeclaration(_) => {
                let lowered = lower_stmt(stmt, ctx);
                for s in lowered {
                    // Arrow function initializers become pending functions (already
                    // handled inside lower_stmt), so only collect VarDecl stmts.
                    if matches!(s, Stmt::VarDecl { .. }) {
                        top_level_vars.push(s);
                    }
                }
            }
            // Import declarations are handled at the module graph level
            Statement::ImportDeclaration(_) => {}
            _ => {}
        }
    }

    // Append any lambda functions generated from arrow expressions
    functions.extend(ctx.pending_functions.drain(..));

    // Monomorphization pass: stamp out specialized copies of generic functions.
    let pending = std::mem::take(&mut ctx.pending_monomorphizations);
    for (generic_name, _type_param_names, concrete_types, mangled_name) in pending {
        if let Some(&stmt_idx) = ctx.generic_fn_indices.get(&generic_name) {
            let func_ast = match &program.body[stmt_idx] {
                Statement::FunctionDeclaration(func) => Some(func.as_ref()),
                Statement::ExportNamedDeclaration(export) => {
                    if let Some(Declaration::FunctionDeclaration(func)) = &export.declaration {
                        Some(func.as_ref())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(func) = func_ast {
                let type_param_names: Vec<String> = func
                    .type_parameters
                    .as_ref()
                    .map(|tp| tp.params.iter().map(|p| p.name.name.to_string()).collect())
                    .unwrap_or_default();
                let generics: HashMap<String, LltsType> = type_param_names
                    .iter()
                    .zip(concrete_types.iter())
                    .map(|(name, ty)| (name.clone(), ty.clone()))
                    .collect();
                if let Some(specialized) = lower_generic_function(func, ctx, &mangled_name, &generics) {
                    ctx.fn_ret_types.insert(mangled_name.clone(), specialized.ret_type.clone());
                    functions.push(specialized);
                }
            }
        }
    }

    // Prepend top-level variable declarations to main's body.
    if !top_level_vars.is_empty() {
        if let Some(main_fn) = functions.iter_mut().find(|f| f.name == "main") {
            top_level_vars.append(&mut main_fn.body);
            main_fn.body = top_level_vars;
        }
    }

    ProgramIR {
        structs,
        enums,
        functions,
    }
}

fn lower_function(func: &Function<'_>, ctx: &mut LowerCtx) -> Option<FunctionDecl> {
    let name = func
        .id
        .as_ref()
        .map(|id| id.name.to_string())?;

    let enum_names = ctx.enum_names();

    // Save state and restore after function (each function gets its own scope)
    let saved_vars = ctx.var_types.clone();

    // Track current function return type for return-statement struct inference
    let fn_ret = ctx.fn_ret_types.get(&name).cloned().unwrap_or(LltsType::Void);
    ctx.var_types.insert("__fn_return_type__".to_string(), fn_ret);

    let params: Vec<(String, LltsType)> = func
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
            // Resolve struct types to include field info, or discriminated union types.
            let pty = match &pty {
                LltsType::Struct { name, fields } if fields.is_empty() => {
                    if let Some(du) = ctx.discriminated_unions.get(name) {
                        du.union_type.clone()
                    } else {
                        ctx.full_struct_type(name)
                    }
                }
                other => other.clone(),
            };
            ctx.var_types.insert(pname.clone(), pty.clone());
            (pname, pty)
        })
        .collect();

    let ret_type = func
        .return_type
        .as_ref()
        .map(|r| lower_ts_type_with_enums(&r.type_annotation, &enum_names))
        .unwrap_or(LltsType::Void);
    let ret_type = match &ret_type {
        LltsType::Struct { name, fields } if fields.is_empty() => {
            if let Some(du) = ctx.discriminated_unions.get(name) {
                du.union_type.clone()
            } else {
                ctx.full_struct_type(name)
            }
        }
        other => other.clone(),
    };

    let body = func
        .body
        .as_ref()
        .map(|b| lower_stmts(&b.statements, ctx))
        .unwrap_or_default();

    ctx.var_types = saved_vars;

    Some(FunctionDecl {
        name,
        params,
        ret_type,
        body,
    })
}



