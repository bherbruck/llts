pub mod call;
pub mod expr;
pub mod intrinsics;
pub mod memory;
pub mod narrowing;
pub mod stmt;
pub mod types;

use std::collections::HashMap;

use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;

use crate::call::CallCodegen;
use crate::expr::{BinOp, ExprCodegen, LogicalOp, UnaryOp};
use crate::intrinsics::Intrinsics;
use crate::memory::MemoryManager;
use crate::stmt::StmtCodegen;
use crate::types::{LltsType, TypeRegistry};

/// A function declaration in the compiler IR, describing its signature and body.
///
/// When the analysis crate stabilizes, this will be replaced by its IR types.
/// For now codegen defines its own lightweight representation.
#[derive(Debug, Clone)]
pub struct FunctionDecl {
    /// Mangled name (e.g. `Point_distance` for methods, `add` for free fns).
    pub name: String,
    /// Parameter names and types.
    pub params: Vec<(String, LltsType)>,
    /// Return type.
    pub ret_type: LltsType,
    /// The body is a list of statements (see [`Stmt`]).
    pub body: Vec<Stmt>,
}

/// A struct (or class/interface) declaration.
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<(String, LltsType)>,
}

/// An enum / tagged union declaration.
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<(String, LltsType)>,
}

/// Statement IR — simplified representation of statements for codegen.
///
/// This is the interface between the analysis pass and codegen. The analysis
/// pass lowers the oxc AST into these typed, validated IR nodes.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `let name: ty = init;` or `const name: ty = init;`
    VarDecl {
        name: String,
        ty: LltsType,
        init: Option<Expr>,
    },
    /// `name = value;`
    Assign {
        target: String,
        value: Expr,
    },
    /// `if (cond) { then } else { else }`
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Option<Vec<Stmt>>,
    },
    /// `while (cond) { body }`
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    /// `for (init; cond; update) { body }`
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        update: Option<Box<Stmt>>,
        body: Vec<Stmt>,
    },
    /// `for (const elem of array) { body }`
    ForOf {
        elem_name: String,
        elem_type: LltsType,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    /// `return expr;` or `return;`
    Return(Option<Expr>),
    /// Block statement: `{ ... }`
    Block(Vec<Stmt>),
    /// Expression statement (e.g. function call as statement).
    Expr(Expr),
    /// `switch (discriminant) { case X: ... default: ... }`
    Switch {
        discriminant: Expr,
        cases: Vec<(Option<Expr>, Vec<Stmt>)>,
    },
    /// `break;`
    Break,
    /// `continue;`
    Continue,
    /// `throw expr;`
    Throw(Expr),
    /// `try { ... } catch (e) { ... }`
    TryCatch {
        try_body: Vec<Stmt>,
        catch_param: Option<String>,
        catch_body: Vec<Stmt>,
    },
}

/// Expression IR — simplified representation for codegen.
#[derive(Debug, Clone)]
pub enum Expr {
    /// Integer literal.
    IntLit { value: i64, ty: LltsType },
    /// Float literal.
    FloatLit { value: f64, ty: LltsType },
    /// Boolean literal.
    BoolLit(bool),
    /// String literal.
    StringLit(String),
    /// Variable reference.
    Var { name: String, ty: LltsType },
    /// Binary operation.
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        ty: LltsType,
    },
    /// Unary operation.
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        ty: LltsType,
    },
    /// Direct function call.
    Call {
        callee: String,
        args: Vec<Expr>,
        ret_type: LltsType,
    },
    /// Method call: `receiver.method(args)`.
    MethodCall {
        class_name: String,
        method_name: String,
        receiver: Box<Expr>,
        args: Vec<Expr>,
        ret_type: LltsType,
    },
    /// Constructor call: `new ClassName(args)`.
    ConstructorCall {
        class_name: String,
        args: Vec<Expr>,
        ret_type: LltsType,
    },
    /// Struct field access: `obj.field`.
    FieldAccess {
        object: Box<Expr>,
        object_type: LltsType,
        field_index: u32,
        field_type: LltsType,
    },
    /// Array indexing: `arr[index]`.
    ArrayIndex {
        array: Box<Expr>,
        index: Box<Expr>,
        elem_type: LltsType,
    },
    /// Type cast: `value as TargetType`.
    Cast {
        value: Box<Expr>,
        from: LltsType,
        to: LltsType,
    },
    /// Struct literal: `{ field1: val1, field2: val2 }`.
    StructLit {
        struct_type: LltsType,
        fields: Vec<Expr>,
    },
    /// Array literal: `[a, b, c]`.
    ArrayLit {
        elem_type: LltsType,
        elements: Vec<Expr>,
    },
    /// Fat pointer call (call through function value).
    IndirectCall {
        callee: Box<Expr>,
        args: Vec<Expr>,
        param_types: Vec<LltsType>,
        ret_type: LltsType,
    },
    /// Ternary conditional: `cond ? then_expr : else_expr`.
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        ty: LltsType,
    },
    /// String concatenation from template literals: `Hello ${name}!`.
    StringConcat {
        parts: Vec<Expr>,
    },
    /// Logical operation with short-circuit evaluation: `a && b`, `a || b`.
    Logical {
        op: LogicalOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        ty: LltsType,
    },
    /// Option<T> None value (null literal assigned to an Option type).
    OptionNone { inner_type: LltsType },
    /// Option<T> Some value (non-null value assigned to an Option type).
    OptionSome { value: Box<Expr>, inner_type: LltsType },
    /// Null check: true if option is Some (x !== null), false if None (x === null).
    OptionIsSome { value: Box<Expr>, inner_type: LltsType },
    /// Unwrap an Option<T>, extracting the inner T value.
    OptionUnwrap { value: Box<Expr>, inner_type: LltsType },
    /// Discriminated union literal: construct a tagged union value from a tag and payload fields.
    UnionLit {
        tag: u32,
        payload: Box<Expr>,
        union_type: LltsType,
    },
}

/// Top-level program IR — the full compilation unit.
#[derive(Debug, Clone)]
pub struct ProgramIR {
    pub structs: Vec<StructDecl>,
    pub enums: Vec<EnumDecl>,
    pub functions: Vec<FunctionDecl>,
}

/// The main code generator. Holds LLVM context, module, builder, and all
/// sub-systems (type registry, memory manager, intrinsics).
///
/// Usage:
/// ```ignore
/// let context = Context::create();
/// let mut codegen = CodeGenerator::new(&context, "main");
/// codegen.compile(&program_ir);
/// let module = codegen.into_module();
/// ```
pub struct CodeGenerator<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    registry: TypeRegistry<'ctx>,
    memory: MemoryManager<'ctx>,
    intrinsics: Intrinsics<'ctx>,
    /// Map from variable name to (alloca pointer, type).
    variables: HashMap<String, (PointerValue<'ctx>, LltsType)>,
    /// Map from function name to LLVM FunctionValue.
    functions: HashMap<String, FunctionValue<'ctx>>,
    /// The currently compiling function (for appending basic blocks).
    current_function: Option<FunctionValue<'ctx>>,
    /// Break target stack (for nested loops).
    break_targets: Vec<BasicBlock<'ctx>>,
    /// Continue target stack (for nested loops).
    continue_targets: Vec<BasicBlock<'ctx>>,
    /// Stack of jmp_buf pointers for try/catch (setjmp/longjmp).
    jmp_buf_stack: Vec<PointerValue<'ctx>>,
}

impl<'ctx> CodeGenerator<'ctx> {
    /// Create a new code generator for a module.
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        let registry = TypeRegistry::new(context);
        let memory = MemoryManager::new(context);
        let intrinsics = Intrinsics::new(context);

        Self {
            context,
            module,
            builder,
            registry,
            memory,
            intrinsics,
            variables: HashMap::new(),
            functions: HashMap::new(),
            current_function: None,
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            jmp_buf_stack: Vec::new(),
        }
    }

    /// Run the 3-pass compilation on a program IR.
    pub fn compile(&mut self, program: &ProgramIR) {
        // Declare intrinsics (malloc, free, write, etc.).
        self.memory.get_or_declare_malloc(&self.module);
        self.memory.get_or_declare_free(&self.module);
        self.intrinsics.declare_all(&self.module);

        // Pass 1: Declare struct types.
        self.pass1_declarations(program);

        // Pass 2: Declare function signatures.
        self.pass2_signatures(program);

        // Pass 3: Emit function bodies.
        self.pass3_bodies(program);
    }

    /// Consume the code generator and return the LLVM module.
    pub fn into_module(self) -> Module<'ctx> {
        self.module
    }

    /// Return a reference to the module (e.g. for verification).
    pub fn module(&self) -> &Module<'ctx> {
        &self.module
    }

    // ---- Pass 1: Declarations ----

    fn pass1_declarations(&mut self, program: &ProgramIR) {
        // Register struct types.
        for s in &program.structs {
            self.registry.declare_struct(&s.name);
        }

        // Define struct bodies.
        for s in &program.structs {
            self.registry.define_struct(&s.name, &s.fields);
        }

        // Register enum / union types.
        for _e in &program.enums {
            // Enums are represented as tagged unions.
            // Nothing extra to declare; they'll be created on demand via
            // TypeRegistry::union_type when referenced.
        }
    }

    // ---- Pass 2: Function Signatures ----

    fn pass2_signatures(&mut self, program: &ProgramIR) {
        for func in &program.functions {
            let param_types: Vec<LltsType> =
                func.params.iter().map(|(_, ty)| ty.clone()).collect();

            // The C runtime expects `int main(void)`, so override void → i32.
            let ret_type = if func.name == "main" && matches!(func.ret_type, LltsType::Void) {
                LltsType::I32
            } else {
                func.ret_type.clone()
            };
            let fn_type = self.registry.fn_type(&param_types, &ret_type);

            let function = self.module.add_function(&func.name, fn_type, None);

            // Name the parameters.
            for (i, (name, _)) in func.params.iter().enumerate() {
                if let Some(param) = function.get_nth_param(i as u32) {
                    param.set_name(name);
                }
            }

            self.functions.insert(func.name.clone(), function);
        }
    }

    // ---- Pass 3: Function Bodies ----

    fn pass3_bodies(&mut self, program: &ProgramIR) {
        for func in &program.functions {
            let function = self.functions[&func.name];
            self.current_function = Some(function);

            // Save outer variable scope.
            let outer_vars = self.variables.clone();

            // Create entry block.
            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);

            // Bind parameters to allocas.
            for (i, (name, ty)) in func.params.iter().enumerate() {
                let param_val = function.get_nth_param(i as u32).unwrap();
                let alloca = self.builder.build_alloca(
                    self.registry.llvm_type(ty),
                    name,
                ).unwrap();
                self.builder.build_store(alloca, param_val).unwrap();
                self.variables.insert(name.clone(), (alloca, ty.clone()));
            }

            // Emit body.
            for s in &func.body {
                self.emit_stmt(s);
            }

            // If there's no terminator, add one.
            if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                if func.name == "main" && matches!(func.ret_type, LltsType::Void) {
                    // main() returns i32 0 to the C runtime.
                    let zero = self.context.i32_type().const_int(0, false);
                    self.builder.build_return(Some(&zero)).unwrap();
                } else if matches!(func.ret_type, LltsType::Void) {
                    self.builder.build_return(None).unwrap();
                } else {
                    // Missing return in non-void function — emit unreachable.
                    self.builder.build_unreachable().unwrap();
                }
            }

            // Restore outer variable scope.
            self.variables = outer_vars;
            self.current_function = None;
        }
    }

    // ---- Statement emission ----

    fn emit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl { name, ty, init } => {
                let init_val = init.as_ref().map(|e| self.emit_expr(e));
                let alloca = StmtCodegen::build_var_decl(
                    &self.builder,
                    &mut self.registry,
                    ty,
                    name,
                    init_val,
                );
                self.variables.insert(name.clone(), (alloca, ty.clone()));
            }
            Stmt::Assign { target, value } => {
                let val = self.emit_expr(value);
                let (ptr, _) = self.variables[target].clone();
                StmtCodegen::build_assignment(&self.builder, ptr, val);
            }
            Stmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let cond = self.emit_expr(condition).into_int_value();
                let function = self.current_function.unwrap();

                // Clone the bodies to avoid borrow conflicts.
                let then_stmts = then_body.clone();
                let else_stmts = else_body.clone();

                let then_bb = self.context.append_basic_block(function, "then");
                let else_bb = self.context.append_basic_block(function, "else");
                let merge_bb = self.context.append_basic_block(function, "merge");

                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)
                    .unwrap();

                // Then.
                self.builder.position_at_end(then_bb);
                for s in &then_stmts {
                    self.emit_stmt(s);
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                // Else.
                self.builder.position_at_end(else_bb);
                if let Some(stmts) = &else_stmts {
                    for s in stmts {
                        self.emit_stmt(s);
                    }
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                self.builder.position_at_end(merge_bb);
            }
            Stmt::While { condition, body } => {
                let function = self.current_function.unwrap();
                let cond_expr = condition.clone();
                let body_stmts = body.clone();

                let cond_bb = self.context.append_basic_block(function, "while_cond");
                let body_bb = self.context.append_basic_block(function, "while_body");
                let end_bb = self.context.append_basic_block(function, "while_end");

                self.break_targets.push(end_bb);
                self.continue_targets.push(cond_bb);

                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(cond_bb);
                let cond = self.emit_expr(&cond_expr).into_int_value();
                self.builder
                    .build_conditional_branch(cond, body_bb, end_bb)
                    .unwrap();

                self.builder.position_at_end(body_bb);
                for s in &body_stmts {
                    self.emit_stmt(s);
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(cond_bb).unwrap();
                }

                self.break_targets.pop();
                self.continue_targets.pop();

                self.builder.position_at_end(end_bb);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                let function = self.current_function.unwrap();

                // Init.
                if let Some(init_stmt) = init {
                    self.emit_stmt(init_stmt);
                }

                let cond_bb = self.context.append_basic_block(function, "for_cond");
                let body_bb = self.context.append_basic_block(function, "for_body");
                let update_bb = self.context.append_basic_block(function, "for_update");
                let end_bb = self.context.append_basic_block(function, "for_end");

                self.break_targets.push(end_bb);
                self.continue_targets.push(update_bb);

                self.builder.build_unconditional_branch(cond_bb).unwrap();

                // Condition.
                self.builder.position_at_end(cond_bb);
                if let Some(cond_expr) = condition {
                    let cond_expr = cond_expr.clone();
                    let cond = self.emit_expr(&cond_expr).into_int_value();
                    self.builder
                        .build_conditional_branch(cond, body_bb, end_bb)
                        .unwrap();
                } else {
                    self.builder
                        .build_unconditional_branch(body_bb)
                        .unwrap();
                }

                // Body.
                self.builder.position_at_end(body_bb);
                let body_stmts = body.clone();
                for s in &body_stmts {
                    self.emit_stmt(s);
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder
                        .build_unconditional_branch(update_bb)
                        .unwrap();
                }

                // Update.
                self.builder.position_at_end(update_bb);
                if let Some(upd) = update {
                    self.emit_stmt(upd);
                }
                self.builder
                    .build_unconditional_branch(cond_bb)
                    .unwrap();

                self.break_targets.pop();
                self.continue_targets.pop();

                self.builder.position_at_end(end_bb);
            }
            Stmt::ForOf {
                elem_name,
                elem_type,
                iterable,
                body,
            } => {
                let function = self.current_function.unwrap();
                let arr_val = self.emit_expr(iterable);

                let i64_ty = self.context.i64_type();
                let elem_llvm_ty = self.registry.llvm_type(elem_type);

                let arr = arr_val.into_struct_value();
                let data_ptr = self.builder
                    .build_extract_value(arr, 0, "forof_data")
                    .unwrap()
                    .into_pointer_value();
                let len = self.builder
                    .build_extract_value(arr, 1, "forof_len")
                    .unwrap()
                    .into_int_value();

                let idx_alloca = self.builder.build_alloca(i64_ty, "forof_i").unwrap();
                self.builder
                    .build_store(idx_alloca, i64_ty.const_int(0, false))
                    .unwrap();

                let cond_bb = self.context.append_basic_block(function, "forof_cond");
                let body_bb = self.context.append_basic_block(function, "forof_body");
                let end_bb = self.context.append_basic_block(function, "forof_end");

                self.break_targets.push(end_bb);
                self.continue_targets.push(cond_bb);

                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(cond_bb);
                let idx = self.builder
                    .build_load(i64_ty, idx_alloca, "i")
                    .unwrap()
                    .into_int_value();
                let cond = self.builder
                    .build_int_compare(
                        inkwell::IntPredicate::ULT,
                        idx,
                        len,
                        "forof_cmp",
                    )
                    .unwrap();
                self.builder
                    .build_conditional_branch(cond, body_bb, end_bb)
                    .unwrap();

                // Body.
                self.builder.position_at_end(body_bb);
                let elem_ptr = unsafe {
                    self.builder
                        .build_gep(elem_llvm_ty, data_ptr, &[idx], "elem_ptr")
                        .unwrap()
                };
                let elem = self.builder
                    .build_load(elem_llvm_ty, elem_ptr, "elem")
                    .unwrap();
                // Bind element variable.
                let elem_alloca = self.builder
                    .build_alloca(elem_llvm_ty, elem_name)
                    .unwrap();
                self.builder.build_store(elem_alloca, elem).unwrap();
                self.variables
                    .insert(elem_name.clone(), (elem_alloca, elem_type.clone()));

                let body_stmts = body.clone();
                for s in &body_stmts {
                    self.emit_stmt(s);
                }

                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    let one = i64_ty.const_int(1, false);
                    let next = self.builder.build_int_add(idx, one, "next_i").unwrap();
                    self.builder.build_store(idx_alloca, next).unwrap();
                    self.builder.build_unconditional_branch(cond_bb).unwrap();
                }

                self.break_targets.pop();
                self.continue_targets.pop();

                self.builder.position_at_end(end_bb);
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    let val = self.emit_expr(e);
                    self.builder.build_return(Some(&val)).unwrap();
                } else {
                    self.builder.build_return(None).unwrap();
                }
            }
            Stmt::Block(stmts) => {
                let outer_vars = self.variables.clone();
                for s in stmts {
                    self.emit_stmt(s);
                }
                self.variables = outer_vars;
            }
            Stmt::Expr(expr) => {
                self.emit_expr(expr);
            }
            Stmt::Switch {
                discriminant,
                cases,
            } => {
                let function = self.current_function.unwrap();
                let disc_val = self.emit_expr(discriminant);
                let cases = cases.clone();

                let end_bb = self.context.append_basic_block(function, "switch_end");

                // Push end_bb as break target so `break` inside cases exits the switch.
                self.break_targets.push(end_bb);

                // Separate default case from valued cases.
                let mut valued_cases: Vec<&(Option<Expr>, Vec<Stmt>)> = Vec::new();
                let mut default_case: Option<&Vec<Stmt>> = None;
                for case in &cases {
                    match &case.0 {
                        Some(_) => valued_cases.push(case),
                        None => default_case = Some(&case.1),
                    }
                }

                // Create basic blocks for each valued case body + default + fallthrough.
                let default_bb = self.context.append_basic_block(function, "switch_default");
                let mut case_bbs: Vec<BasicBlock<'ctx>> = Vec::new();
                for (i, _) in valued_cases.iter().enumerate() {
                    let bb = self.context.append_basic_block(function, &format!("switch_case_{i}"));
                    case_bbs.push(bb);
                }

                // Build comparison chain: for each case, compare and branch.
                // First case test starts from the current block.
                for (i, case) in valued_cases.iter().enumerate() {
                    let case_val = self.emit_expr(case.0.as_ref().unwrap());
                    let next_test_bb = if i + 1 < valued_cases.len() {
                        self.context.append_basic_block(function, &format!("switch_test_{}", i + 1))
                    } else {
                        // Last case: fall through to default.
                        default_bb
                    };

                    // Compare discriminant == case value.
                    let cmp = if disc_val.is_int_value() && case_val.is_int_value() {
                        self.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                disc_val.into_int_value(),
                                case_val.into_int_value(),
                                "switch_cmp",
                            )
                            .unwrap()
                    } else if disc_val.is_float_value() && case_val.is_float_value() {
                        self.builder
                            .build_float_compare(
                                inkwell::FloatPredicate::OEQ,
                                disc_val.into_float_value(),
                                case_val.into_float_value(),
                                "switch_cmp",
                            )
                            .unwrap()
                    } else {
                        // Fallback: int compare (covers bool and other int types).
                        self.builder
                            .build_int_compare(
                                inkwell::IntPredicate::EQ,
                                disc_val.into_int_value(),
                                case_val.into_int_value(),
                                "switch_cmp",
                            )
                            .unwrap()
                    };

                    self.builder
                        .build_conditional_branch(cmp, case_bbs[i], next_test_bb)
                        .unwrap();

                    // Position at next test block for the next iteration.
                    if i + 1 < valued_cases.len() {
                        self.builder.position_at_end(next_test_bb);
                    }
                }

                // Emit case bodies. Each case falls through to the next case body
                // unless a break (or return) terminates the block.
                for (i, case) in valued_cases.iter().enumerate() {
                    self.builder.position_at_end(case_bbs[i]);
                    for s in &case.1 {
                        self.emit_stmt(s);
                    }
                    // Fall through to next case body, default, or end.
                    if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                        let next = if i + 1 < case_bbs.len() {
                            case_bbs[i + 1]
                        } else {
                            default_bb
                        };
                        self.builder.build_unconditional_branch(next).unwrap();
                    }
                }

                // Emit default case body.
                self.builder.position_at_end(default_bb);
                if let Some(body) = default_case {
                    for s in body {
                        self.emit_stmt(s);
                    }
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(end_bb).unwrap();
                }

                self.break_targets.pop();

                // If there were no valued cases, branch from current position to default.
                // (Already handled by comparison chain above for non-empty cases.)

                self.builder.position_at_end(end_bb);
            }
            Stmt::Break => {
                if let Some(&target) = self.break_targets.last() {
                    self.builder.build_unconditional_branch(target).unwrap();
                }
            }
            Stmt::Continue => {
                if let Some(&target) = self.continue_targets.last() {
                    self.builder.build_unconditional_branch(target).unwrap();
                }
            }
            Stmt::Throw(expr) => {
                let val = self.emit_expr(expr);

                // Write the thrown value to stderr as a string.
                let ty = self.infer_expr_type(expr);
                let str_val = if matches!(ty, LltsType::String) {
                    val
                } else {
                    // Convert to "[object]\n" fallback.
                    ExprCodegen::const_string(
                        &self.builder,
                        &self.module,
                        self.context,
                        &self.registry,
                        "[thrown]",
                        "thrown_str",
                    )
                };

                // Write error message to stderr.
                self.intrinsics.build_write_stderr(&self.builder, &self.module, str_val);
                // Write newline to stderr.
                let newline = ExprCodegen::const_string(
                    &self.builder,
                    &self.module,
                    self.context,
                    &self.registry,
                    "\n",
                    "newline",
                );
                self.intrinsics.build_write_stderr(&self.builder, &self.module, newline);

                if let Some(&jmp_buf_ptr) = self.jmp_buf_stack.last() {
                    // Inside a try block: longjmp back to the catch handler.
                    let longjmp = self.intrinsics.get("longjmp").expect("longjmp not declared");
                    let one = self.context.i32_type().const_int(1, false);
                    self.builder
                        .build_call(longjmp, &[jmp_buf_ptr.into(), one.into()], "")
                        .unwrap();
                    self.builder.build_unreachable().unwrap();
                } else {
                    // No enclosing try block: call exit(1).
                    let exit_fn = self.intrinsics.get("exit").expect("exit not declared");
                    let one = self.context.i32_type().const_int(1, false);
                    self.builder
                        .build_call(exit_fn, &[one.into()], "")
                        .unwrap();
                    self.builder.build_unreachable().unwrap();
                }
            }
            Stmt::TryCatch {
                try_body,
                catch_param,
                catch_body,
            } => {
                let function = self.current_function.unwrap();
                let try_stmts = try_body.clone();
                let catch_stmts = catch_body.clone();
                let catch_param = catch_param.clone();

                // Allocate a jmp_buf on the stack (200 bytes, enough for any platform).
                let i8_ty = self.context.i8_type();
                let jmp_buf_ty = i8_ty.array_type(200);
                let jmp_buf_alloca = self.builder.build_alloca(jmp_buf_ty, "jmp_buf").unwrap();
                let ptr_ty = self.context.ptr_type(AddressSpace::default());
                let jmp_buf_ptr = self.builder
                    .build_pointer_cast(jmp_buf_alloca, ptr_ty, "jmp_buf_ptr")
                    .unwrap();

                // Call setjmp(jmp_buf). Returns 0 on first call, non-zero from longjmp.
                let setjmp = self.intrinsics.get("setjmp").expect("setjmp not declared");
                let setjmp_result = self.builder
                    .build_call(setjmp, &[jmp_buf_ptr.into()], "setjmp_res")
                    .unwrap()
                    .try_as_basic_value()
                    .unwrap_basic()
                    .into_int_value();

                let zero = self.context.i32_type().const_int(0, false);
                let is_normal = self.builder
                    .build_int_compare(inkwell::IntPredicate::EQ, setjmp_result, zero, "is_normal")
                    .unwrap();

                let try_bb = self.context.append_basic_block(function, "try_body");
                let catch_bb = self.context.append_basic_block(function, "catch_body");
                let merge_bb = self.context.append_basic_block(function, "try_merge");

                self.builder
                    .build_conditional_branch(is_normal, try_bb, catch_bb)
                    .unwrap();

                // Try body: push jmp_buf so throw can find it.
                self.builder.position_at_end(try_bb);
                self.jmp_buf_stack.push(jmp_buf_ptr);
                for s in &try_stmts {
                    if self.builder.get_insert_block().unwrap().get_terminator().is_some() {
                        break;
                    }
                    self.emit_stmt(s);
                }
                self.jmp_buf_stack.pop();
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                // Catch body.
                self.builder.position_at_end(catch_bb);
                if let Some(param_name) = &catch_param {
                    // Bind catch parameter as a string with value "error".
                    // (In v1, we don't carry the thrown value through longjmp.)
                    let err_str = ExprCodegen::const_string(
                        &self.builder,
                        &self.module,
                        self.context,
                        &self.registry,
                        "error",
                        "catch_err",
                    );
                    let str_ty = self.registry.llvm_type(&LltsType::String);
                    let alloca = self.builder.build_alloca(str_ty, param_name).unwrap();
                    self.builder.build_store(alloca, err_str).unwrap();
                    self.variables.insert(param_name.clone(), (alloca, LltsType::String));
                }
                for s in &catch_stmts {
                    if self.builder.get_insert_block().unwrap().get_terminator().is_some() {
                        break;
                    }
                    self.emit_stmt(s);
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                self.builder.position_at_end(merge_bb);
            }
        }
    }

    // ---- Expression emission ----

    fn emit_expr(&mut self, expr: &Expr) -> BasicValueEnum<'ctx> {
        match expr {
            Expr::IntLit { value, ty } => match ty {
                LltsType::I32 | LltsType::U32 => ExprCodegen::const_i32(self.context, *value),
                LltsType::I64 | LltsType::U64 => ExprCodegen::const_i64(self.context, *value),
                LltsType::I8 | LltsType::U8 => {
                    self.context
                        .i8_type()
                        .const_int(*value as u64, *value < 0)
                        .into()
                }
                LltsType::I16 | LltsType::U16 => {
                    self.context
                        .i16_type()
                        .const_int(*value as u64, *value < 0)
                        .into()
                }
                _ => ExprCodegen::const_i32(self.context, *value),
            },
            Expr::FloatLit { value, ty } => match ty {
                LltsType::F32 => ExprCodegen::const_f32(self.context, *value),
                _ => ExprCodegen::const_f64(self.context, *value),
            },
            Expr::BoolLit(v) => ExprCodegen::const_bool(self.context, *v),
            Expr::StringLit(s) => ExprCodegen::const_string(
                &self.builder,
                &self.module,
                self.context,
                &self.registry,
                s,
                "str",
            ),
            Expr::Var { name, ty: _ } => {
                let (ptr, var_ty) = self.variables[name].clone();
                let llvm_ty = self.registry.llvm_type(&var_ty);
                self.builder
                    .build_load(llvm_ty, ptr, name)
                    .unwrap()
            }
            Expr::Binary { op, lhs, rhs, ty } => {
                let lhs_ty = self.infer_expr_type(lhs);
                let rhs_ty = self.infer_expr_type(rhs);
                let l = self.emit_expr(lhs);
                let r = self.emit_expr(rhs);

                // Implicit widening: promote operands to a common type.
                let (l, r, effective_ty) =
                    self.coerce_binary_operands(l, r, &lhs_ty, &rhs_ty, ty);

                ExprCodegen::build_binary(
                    &self.builder,
                    self.context,
                    *op,
                    l,
                    r,
                    &effective_ty,
                    "binop",
                )
            }
            Expr::Unary { op, operand, ty } => {
                let v = self.emit_expr(operand);
                ExprCodegen::build_unary(&self.builder, self.context, *op, v, ty, "unary")
            }
            Expr::Call {
                callee,
                args,
                ret_type: _,
            } => {
                // Check for built-in print / console.log.
                if callee == "print" || callee == "console_log" {
                    return self.emit_print_call(args);
                }

                // Check for Math.* intrinsics (lowered as Math_sqrt, Math_floor, etc.)
                if let Some(math_fn) = callee.strip_prefix("Math_") {
                    let arg_vals: Vec<BasicValueEnum<'ctx>> =
                        args.iter().map(|a| self.emit_expr(a)).collect();
                    let intrinsic_name = match math_fn {
                        "sqrt" => "llvm.sqrt.f64",
                        "abs" => "llvm.fabs.f64",
                        "floor" => "llvm.floor.f64",
                        "ceil" => "llvm.ceil.f64",
                        "sin" => "sin",
                        "cos" => "cos",
                        "log" => "log",
                        "exp" => "exp",
                        "pow" => {
                            if arg_vals.len() == 2 {
                                return self
                                    .intrinsics
                                    .build_math_pow(
                                        &self.builder,
                                        arg_vals[0],
                                        arg_vals[1],
                                        "pow",
                                    )
                                    .expect("pow intrinsic failed");
                            }
                            panic!("Math.pow requires 2 arguments");
                        }
                        other => panic!("unknown Math function: {other}"),
                    };
                    return self
                        .intrinsics
                        .build_math_unary(&self.builder, intrinsic_name, arg_vals[0], math_fn)
                        .expect("math intrinsic failed");
                }

                let arg_vals: Vec<BasicValueEnum<'ctx>> =
                    args.iter().map(|a| self.emit_expr(a)).collect();

                let function = self.functions.get(callee).copied().unwrap_or_else(|| {
                    self.module
                        .get_function(callee)
                        .unwrap_or_else(|| panic!("function not found: {callee}"))
                });

                match CallCodegen::build_direct_call(
                    &self.builder,
                    function,
                    &arg_vals,
                    "call",
                ) {
                    Some(v) => v,
                    None => {
                        // Void return — return a dummy value.
                        self.context.i8_type().const_int(0, false).into()
                    }
                }
            }
            Expr::MethodCall {
                class_name,
                method_name,
                receiver,
                args,
                ret_type: _,
            } => {
                let recv = self.emit_expr(receiver);
                let arg_vals: Vec<BasicValueEnum<'ctx>> =
                    args.iter().map(|a| self.emit_expr(a)).collect();

                CallCodegen::build_method_call(
                    &self.builder,
                    &self.module,
                    class_name,
                    method_name,
                    recv,
                    &arg_vals,
                    "method",
                )
                .unwrap_or_else(|| self.context.i8_type().const_int(0, false).into())
            }
            Expr::ConstructorCall {
                class_name,
                args,
                ret_type: _,
            } => {
                let arg_vals: Vec<BasicValueEnum<'ctx>> =
                    args.iter().map(|a| self.emit_expr(a)).collect();

                CallCodegen::build_constructor_call(
                    &self.builder,
                    &self.module,
                    class_name,
                    &arg_vals,
                    "new",
                )
                .unwrap_or_else(|| self.context.i8_type().const_int(0, false).into())
            }
            Expr::FieldAccess {
                object,
                object_type,
                field_index,
                field_type,
            } => {
                let obj = self.emit_expr(object);
                // If the object is a struct value (not a pointer), we use
                // extract_value. If it's a pointer, we GEP.
                if obj.is_pointer_value() {
                    ExprCodegen::build_load_struct_field(
                        &self.builder,
                        &mut self.registry,
                        obj.into_pointer_value(),
                        object_type,
                        *field_index,
                        field_type,
                        "field",
                    )
                } else {
                    // Direct struct value — extract.
                    self.builder
                        .build_extract_value(
                            obj.into_struct_value(),
                            *field_index,
                            "field",
                        )
                        .unwrap()
                }
            }
            Expr::ArrayIndex {
                array,
                index,
                elem_type,
            } => {
                let arr = self.emit_expr(array);
                let i64_ty = self.context.i64_type();
                let index_type = self.infer_expr_type(index);
                let raw = self.emit_expr(index);
                let idx = if TypeRegistry::is_float(&index_type) {
                    // f64/f32 index → fptosi to i64
                    self.builder
                        .build_float_to_signed_int(raw.into_float_value(), i64_ty, "idx_cast")
                        .unwrap()
                } else if raw.is_int_value() {
                    let iv = raw.into_int_value();
                    if iv.get_type().get_bit_width() == 64 {
                        iv
                    } else if TypeRegistry::is_signed(&index_type) {
                        self.builder.build_int_s_extend(iv, i64_ty, "idx_sext").unwrap()
                    } else {
                        self.builder.build_int_z_extend(iv, i64_ty, "idx_zext").unwrap()
                    }
                } else {
                    raw.into_int_value() // fallback — will panic if not int
                };
                let function = self.current_function.unwrap();

                ExprCodegen::build_array_index(
                    &self.builder,
                    self.context,
                    &mut self.registry,
                    function,
                    arr,
                    idx,
                    elem_type,
                    "arr_elem",
                )
            }
            Expr::Cast { value, from, to } => {
                let v = self.emit_expr(value);
                ExprCodegen::build_cast(&self.builder, self.context, v, from, to, "cast")
            }
            Expr::StructLit {
                struct_type,
                fields,
            } => {
                let field_vals: Vec<BasicValueEnum<'ctx>> =
                    fields.iter().map(|f| self.emit_expr(f)).collect();
                let ptr = ExprCodegen::build_struct_literal(
                    &self.builder,
                    &mut self.registry,
                    struct_type,
                    &field_vals,
                    "struct_lit",
                );
                // Load the struct value from the alloca.
                let llvm_ty = self.registry.llvm_type(struct_type);
                self.builder
                    .build_load(llvm_ty, ptr, "struct_val")
                    .unwrap()
            }
            Expr::ArrayLit {
                elem_type,
                elements,
            } => {
                let elem_vals: Vec<BasicValueEnum<'ctx>> =
                    elements.iter().map(|e| self.emit_expr(e)).collect();
                ExprCodegen::build_array_literal(
                    &self.builder,
                    self.context,
                    &self.module,
                    &mut self.registry,
                    elem_type,
                    &elem_vals,
                    "arr_lit",
                )
            }
            Expr::IndirectCall {
                callee,
                args,
                param_types,
                ret_type,
            } => {
                let fat_ptr = self.emit_expr(callee);
                let arg_vals: Vec<BasicValueEnum<'ctx>> =
                    args.iter().map(|a| self.emit_expr(a)).collect();

                CallCodegen::build_fat_ptr_call(
                    &self.builder,
                    self.context,
                    &mut self.registry,
                    fat_ptr,
                    &arg_vals,
                    param_types,
                    ret_type,
                    "indirect_call",
                )
                .unwrap_or_else(|| self.context.i8_type().const_int(0, false).into())
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
                ty: _,
            } => {
                let cond = self.emit_expr(condition).into_int_value();
                let function = self.current_function.unwrap();

                let then_expr = then_expr.clone();
                let else_expr = else_expr.clone();

                let then_bb = self.context.append_basic_block(function, "tern_then");
                let else_bb = self.context.append_basic_block(function, "tern_else");
                let merge_bb = self.context.append_basic_block(function, "tern_merge");

                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)
                    .unwrap();

                // Then branch.
                self.builder.position_at_end(then_bb);
                let then_val = self.emit_expr(&then_expr);
                let then_bb_end = self.builder.get_insert_block().unwrap();
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                // Else branch.
                self.builder.position_at_end(else_bb);
                let else_val = self.emit_expr(&else_expr);
                let else_bb_end = self.builder.get_insert_block().unwrap();
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                // Merge with phi.
                self.builder.position_at_end(merge_bb);
                let phi = self
                    .builder
                    .build_phi(then_val.get_type(), "tern_result")
                    .unwrap();
                phi.add_incoming(&[(&then_val, then_bb_end), (&else_val, else_bb_end)]);
                phi.as_basic_value()
            }
            Expr::StringConcat { parts } => {
                let parts = parts.clone();
                let mut result = self.emit_expr_as_string(&parts[0]);
                for part in &parts[1..] {
                    let rhs = self.emit_expr_as_string(part);
                    result = self.intrinsics.build_string_concat(
                        &self.builder,
                        &self.module,
                        &self.registry,
                        result,
                        rhs,
                    );
                }
                result
            }
            Expr::Logical { op, lhs, rhs, ty: _ } => {
                let op = *op;
                let lhs = lhs.clone();
                let rhs = rhs.clone();

                let function = self.current_function.unwrap();
                let bool_ty = self.context.bool_type();

                // Evaluate LHS
                let lhs_val = self.emit_expr(&lhs).into_int_value();
                let lhs_bb = self.builder.get_insert_block().unwrap();

                let rhs_bb = self.context.append_basic_block(function, "logical_rhs");
                let merge_bb = self.context.append_basic_block(function, "logical_merge");

                match op {
                    LogicalOp::And => {
                        // &&: if lhs is false, short-circuit to false; else eval rhs
                        self.builder
                            .build_conditional_branch(lhs_val, rhs_bb, merge_bb)
                            .unwrap();
                    }
                    LogicalOp::Or => {
                        // ||: if lhs is true, short-circuit to true; else eval rhs
                        self.builder
                            .build_conditional_branch(lhs_val, merge_bb, rhs_bb)
                            .unwrap();
                    }
                }

                // Evaluate RHS
                self.builder.position_at_end(rhs_bb);
                let rhs_val = self.emit_expr(&rhs).into_int_value();
                let rhs_end_bb = self.builder.get_insert_block().unwrap();
                self.builder
                    .build_unconditional_branch(merge_bb)
                    .unwrap();

                // Merge with phi
                self.builder.position_at_end(merge_bb);
                let phi = self.builder.build_phi(bool_ty, "logical_result").unwrap();

                match op {
                    LogicalOp::And => {
                        // From lhs_bb (short-circuit): result = false
                        let false_val = bool_ty.const_int(0, false);
                        phi.add_incoming(&[
                            (&false_val, lhs_bb),
                            (&rhs_val, rhs_end_bb),
                        ]);
                    }
                    LogicalOp::Or => {
                        // From lhs_bb (short-circuit): result = true
                        let true_val = bool_ty.const_int(1, false);
                        phi.add_incoming(&[
                            (&true_val, lhs_bb),
                            (&rhs_val, rhs_end_bb),
                        ]);
                    }
                }

                phi.as_basic_value()
            }
            Expr::OptionNone { inner_type } => {
                use crate::narrowing::NarrowingCodegen;
                NarrowingCodegen::build_option_none(
                    &self.builder,
                    self.context,
                    &mut self.registry,
                    inner_type,
                )
            }
            Expr::OptionSome { value, inner_type } => {
                use crate::narrowing::NarrowingCodegen;
                let val = self.emit_expr(value);
                NarrowingCodegen::build_option_some(
                    &self.builder,
                    self.context,
                    &mut self.registry,
                    inner_type,
                    val,
                )
            }
            Expr::OptionIsSome { value, inner_type: _ } => {
                use crate::narrowing::NarrowingCodegen;
                let val = self.emit_expr(value);
                NarrowingCodegen::build_option_is_some(&self.builder, val).into()
            }
            Expr::OptionUnwrap { value, inner_type: _ } => {
                use crate::narrowing::NarrowingCodegen;
                let val = self.emit_expr(value);
                NarrowingCodegen::build_option_unwrap(&self.builder, val)
            }
            Expr::UnionLit { tag, payload, union_type } => {
                use crate::narrowing::NarrowingCodegen;
                let payload_val = self.emit_expr(payload);
                NarrowingCodegen::build_union_value(
                    &self.builder,
                    self.context,
                    &mut self.registry,
                    union_type,
                    *tag,
                    payload_val,
                )
            }
        }
    }

    /// Emit an expression and convert the result to a string fat pointer.
    /// If the expression is already a string, return it directly.
    /// Otherwise, use snprintf to format the value into a heap-allocated buffer.
    fn emit_expr_as_string(&mut self, expr: &Expr) -> BasicValueEnum<'ctx> {
        let ty = self.infer_expr_type(expr);
        let val = self.emit_expr(expr);

        match &ty {
            LltsType::String => val,
            t if TypeRegistry::is_float(t) => {
                // Format f64 with snprintf into a stack buffer, then build { ptr, len }.
                let f64_val: BasicValueEnum<'ctx> = if matches!(t, LltsType::F32) {
                    self.builder
                        .build_float_ext(
                            val.into_float_value(),
                            self.context.f64_type(),
                            "ext",
                        )
                        .unwrap()
                        .into()
                } else {
                    val
                };
                self.build_snprintf_to_string("%.15g", f64_val)
            }
            t if TypeRegistry::is_integer(t) => {
                // Extend to i64 for consistent formatting with %ld.
                let i64_val = if val.into_int_value().get_type().get_bit_width() < 64 {
                    if TypeRegistry::is_signed(t) {
                        self.builder
                            .build_int_s_extend(
                                val.into_int_value(),
                                self.context.i64_type(),
                                "ext",
                            )
                            .unwrap()
                    } else {
                        self.builder
                            .build_int_z_extend(
                                val.into_int_value(),
                                self.context.i64_type(),
                                "ext",
                            )
                            .unwrap()
                    }
                } else {
                    val.into_int_value()
                };
                let fmt = if TypeRegistry::is_unsigned(t) { "%lu" } else { "%ld" };
                self.build_snprintf_to_string(fmt, i64_val.into())
            }
            LltsType::Bool => {
                // Convert bool to "true" or "false" string.
                let function = self.current_function.unwrap();
                let true_bb = self.context.append_basic_block(function, "bool_true");
                let false_bb = self.context.append_basic_block(function, "bool_false");
                let merge_bb = self.context.append_basic_block(function, "bool_merge");

                self.builder
                    .build_conditional_branch(val.into_int_value(), true_bb, false_bb)
                    .unwrap();

                self.builder.position_at_end(true_bb);
                let true_str = ExprCodegen::const_string(
                    &self.builder,
                    &self.module,
                    self.context,
                    &self.registry,
                    "true",
                    "true_str",
                );
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(false_bb);
                let false_str = ExprCodegen::const_string(
                    &self.builder,
                    &self.module,
                    self.context,
                    &self.registry,
                    "false",
                    "false_str",
                );
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(merge_bb);
                let str_ty = self.registry.string_type();
                let phi = self.builder.build_phi(str_ty, "bool_str").unwrap();
                phi.add_incoming(&[(&true_str, true_bb), (&false_str, false_bb)]);
                phi.as_basic_value()
            }
            _ => {
                // Fallback: return "[object]".
                ExprCodegen::const_string(
                    &self.builder,
                    &self.module,
                    self.context,
                    &self.registry,
                    "[object]",
                    "obj_str",
                )
            }
        }
    }

    /// Use snprintf to format a value into a stack buffer, then build a string
    /// fat pointer { ptr, len } from the result.
    fn build_snprintf_to_string(
        &mut self,
        fmt: &str,
        value: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let snprintf = self.intrinsics.get("snprintf").expect("snprintf not declared");

        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(AddressSpace::default());

        // Stack buffer (64 bytes is plenty for numeric formatting).
        let buf = self.builder
            .build_alloca(self.context.i8_type().array_type(64), "fmt_buf")
            .unwrap();
        let buf_ptr = self.builder
            .build_pointer_cast(buf, ptr_ty, "buf_ptr")
            .unwrap();
        let buf_size = i64_ty.const_int(64, false);

        // Format string.
        let fmt_ptr = self.builder
            .build_global_string_ptr(fmt, "fmt")
            .unwrap()
            .as_pointer_value();

        // snprintf(buf, 64, fmt, value)
        let len = self.builder
            .build_call(
                snprintf,
                &[buf_ptr.into(), buf_size.into(), fmt_ptr.into(), value.into()],
                "fmt_len",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();

        // Build { ptr, len } string struct. The buffer is stack-allocated, but
        // build_string_concat will memcpy it into a heap buffer during concat,
        // so this is safe for the lifetime of the concat chain.
        let len_i64 = self.builder
            .build_int_z_extend(len, i64_ty, "len_i64")
            .unwrap();
        let str_ty = self.registry.string_type();
        let str_val = str_ty.get_undef();
        let str_val = self.builder
            .build_insert_value(str_val, buf_ptr, 0, "snprintf_ptr")
            .unwrap()
            .into_struct_value();
        let str_val = self.builder
            .build_insert_value(str_val, len_i64, 1, "snprintf_len")
            .unwrap()
            .into_struct_value();
        str_val.into()
    }

    /// Emit a print/console.log call. Dispatches based on argument type.
    fn emit_print_call(&mut self, args: &[Expr]) -> BasicValueEnum<'ctx> {
        for arg in args {
            let val = self.emit_expr(arg);

            // Detect the type from the expression.
            let ty = self.infer_expr_type(arg);

            match &ty {
                LltsType::String => {
                    self.intrinsics
                        .build_print_string(&self.builder, &self.module, val);
                    // Print newline after the string.
                    let newline = ExprCodegen::const_string(
                        &self.builder,
                        &self.module,
                        self.context,
                        &self.registry,
                        "\n",
                        "newline",
                    );
                    self.intrinsics
                        .build_print_string(&self.builder, &self.module, newline);
                }
                t if TypeRegistry::is_integer(t) => {
                    let unsigned = TypeRegistry::is_unsigned(t);
                    // Extend to i32 if needed for printf.
                    let i32_val = if val.into_int_value().get_type().get_bit_width() < 32 {
                        if unsigned {
                            self.builder
                                .build_int_z_extend(
                                    val.into_int_value(),
                                    self.context.i32_type(),
                                    "ext",
                                )
                                .unwrap()
                        } else {
                            self.builder
                                .build_int_s_extend(
                                    val.into_int_value(),
                                    self.context.i32_type(),
                                    "ext",
                                )
                                .unwrap()
                        }
                    } else {
                        val.into_int_value()
                    };
                    if unsigned {
                        self.intrinsics
                            .build_print_u32(&self.builder, &self.module, i32_val);
                    } else {
                        self.intrinsics
                            .build_print_i32(&self.builder, &self.module, i32_val);
                    }
                }
                t if TypeRegistry::is_float(t) => {
                    let f64_val: BasicValueEnum<'ctx> = if matches!(t, LltsType::F32) {
                        self.builder
                            .build_float_ext(
                                val.into_float_value(),
                                self.context.f64_type(),
                                "ext",
                            )
                            .unwrap()
                            .into()
                    } else {
                        val
                    };
                    self.intrinsics
                        .build_print_f64(&self.builder, &self.module, f64_val);
                }
                LltsType::Bool => {
                    // Print "true" or "false".
                    let function = self.current_function.unwrap();
                    let true_bb = self.context.append_basic_block(function, "print_true");
                    let false_bb = self.context.append_basic_block(function, "print_false");
                    let done_bb = self.context.append_basic_block(function, "print_done");

                    self.builder
                        .build_conditional_branch(val.into_int_value(), true_bb, false_bb)
                        .unwrap();

                    self.builder.position_at_end(true_bb);
                    let true_str = ExprCodegen::const_string(
                        &self.builder,
                        &self.module,
                        self.context,
                        &self.registry,
                        "true\n",
                        "true_str",
                    );
                    self.intrinsics
                        .build_print_string(&self.builder, &self.module, true_str);
                    self.builder
                        .build_unconditional_branch(done_bb)
                        .unwrap();

                    self.builder.position_at_end(false_bb);
                    let false_str = ExprCodegen::const_string(
                        &self.builder,
                        &self.module,
                        self.context,
                        &self.registry,
                        "false\n",
                        "false_str",
                    );
                    self.intrinsics
                        .build_print_string(&self.builder, &self.module, false_str);
                    self.builder
                        .build_unconditional_branch(done_bb)
                        .unwrap();

                    self.builder.position_at_end(done_bb);
                }
                _ => {
                    // Fallback: print "<object>".
                    let fallback = ExprCodegen::const_string(
                        &self.builder,
                        &self.module,
                        self.context,
                        &self.registry,
                        "<object>\n",
                        "obj_str",
                    );
                    self.intrinsics
                        .build_print_string(&self.builder, &self.module, fallback);
                }
            }
        }

        // print returns void; return a dummy.
        self.context.i8_type().const_int(0, false).into()
    }

    /// Implicit widening for binary operations with mismatched types.
    /// Returns (coerced_lhs, coerced_rhs, common_type).
    ///
    /// Widening rules (always safe, no data loss):
    ///   - any int + f64/f32 → both promoted to float
    ///   - narrow int + wide int → both promoted to wider int
    ///   - same type → no-op
    fn coerce_binary_operands(
        &self,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
        lhs_ty: &LltsType,
        rhs_ty: &LltsType,
        declared_ty: &LltsType,
    ) -> (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>, LltsType) {
        // Same type — no coercion needed.
        if lhs_ty == rhs_ty {
            return (lhs, rhs, declared_ty.clone());
        }

        let l_float = TypeRegistry::is_float(lhs_ty);
        let r_float = TypeRegistry::is_float(rhs_ty);
        let l_int = TypeRegistry::is_integer(lhs_ty);
        let r_int = TypeRegistry::is_integer(rhs_ty);

        // One float, one int → promote int to float.
        if l_float && r_int {
            let r2 = ExprCodegen::build_cast(
                &self.builder,
                self.context,
                rhs,
                rhs_ty,
                lhs_ty,
                "widen_r",
            );
            return (lhs, r2, lhs_ty.clone());
        }
        if r_float && l_int {
            let l2 = ExprCodegen::build_cast(
                &self.builder,
                self.context,
                lhs,
                lhs_ty,
                rhs_ty,
                "widen_l",
            );
            return (l2, rhs, rhs_ty.clone());
        }

        // Both int, different widths → promote narrower to wider.
        if l_int && r_int {
            let l_width = TypeRegistry::bit_width(lhs_ty);
            let r_width = TypeRegistry::bit_width(rhs_ty);
            if l_width < r_width {
                let l2 = ExprCodegen::build_cast(
                    &self.builder,
                    self.context,
                    lhs,
                    lhs_ty,
                    rhs_ty,
                    "widen_l",
                );
                return (l2, rhs, rhs_ty.clone());
            } else {
                let r2 = ExprCodegen::build_cast(
                    &self.builder,
                    self.context,
                    rhs,
                    rhs_ty,
                    lhs_ty,
                    "widen_r",
                );
                return (lhs, r2, lhs_ty.clone());
            }
        }

        // Both float, different precision → promote to f64.
        if l_float && r_float && lhs_ty != rhs_ty {
            let target = LltsType::F64;
            let l2 = ExprCodegen::build_cast(
                &self.builder,
                self.context,
                lhs,
                lhs_ty,
                &target,
                "widen_l",
            );
            let r2 = ExprCodegen::build_cast(
                &self.builder,
                self.context,
                rhs,
                rhs_ty,
                &target,
                "widen_r",
            );
            return (l2, r2, target);
        }

        // Fallback: no coercion (let build_binary handle or panic).
        (lhs, rhs, declared_ty.clone())
    }

    /// Infer the LltsType of an expression from the IR.
    fn infer_expr_type(&self, expr: &Expr) -> LltsType {
        match expr {
            Expr::IntLit { ty, .. } => ty.clone(),
            Expr::FloatLit { ty, .. } => ty.clone(),
            Expr::BoolLit(_) => LltsType::Bool,
            Expr::StringLit(_) => LltsType::String,
            Expr::Var { ty, .. } => ty.clone(),
            Expr::Binary { ty, .. } => ty.clone(),
            Expr::Unary { ty, .. } => ty.clone(),
            Expr::Call { ret_type, .. } => ret_type.clone(),
            Expr::MethodCall { ret_type, .. } => ret_type.clone(),
            Expr::ConstructorCall { ret_type, .. } => ret_type.clone(),
            Expr::FieldAccess { field_type, .. } => field_type.clone(),
            Expr::ArrayIndex { elem_type, .. } => elem_type.clone(),
            Expr::Cast { to, .. } => to.clone(),
            Expr::StructLit { struct_type, .. } => struct_type.clone(),
            Expr::ArrayLit { elem_type, .. } => LltsType::Array(Box::new(elem_type.clone())),
            Expr::IndirectCall { ret_type, .. } => ret_type.clone(),
            Expr::Ternary { ty, .. } => ty.clone(),
            Expr::StringConcat { .. } => LltsType::String,
            Expr::Logical { ty, .. } => ty.clone(),
            Expr::OptionNone { inner_type } => LltsType::Option(Box::new(inner_type.clone())),
            Expr::OptionSome { inner_type, .. } => LltsType::Option(Box::new(inner_type.clone())),
            Expr::OptionIsSome { .. } => LltsType::Bool,
            Expr::OptionUnwrap { inner_type, .. } => inner_type.clone(),
            Expr::UnionLit { union_type, .. } => union_type.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_program() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");
        let program = ProgramIR {
            structs: vec![],
            enums: vec![],
            functions: vec![],
        };
        codegen.compile(&program);
        // Should not panic.
        assert!(codegen.module().verify().is_ok());
    }

    #[test]
    fn test_simple_add_function() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");

        let program = ProgramIR {
            structs: vec![],
            enums: vec![],
            functions: vec![FunctionDecl {
                name: "add".to_string(),
                params: vec![
                    ("a".to_string(), LltsType::I32),
                    ("b".to_string(), LltsType::I32),
                ],
                ret_type: LltsType::I32,
                body: vec![Stmt::Return(Some(Expr::Binary {
                    op: BinOp::Add,
                    lhs: Box::new(Expr::Var {
                        name: "a".to_string(),
                        ty: LltsType::I32,
                    }),
                    rhs: Box::new(Expr::Var {
                        name: "b".to_string(),
                        ty: LltsType::I32,
                    }),
                    ty: LltsType::I32,
                }))],
            }],
        };

        codegen.compile(&program);
        assert!(codegen.module().verify().is_ok());
    }

    #[test]
    fn test_if_else() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");

        let program = ProgramIR {
            structs: vec![],
            enums: vec![],
            functions: vec![FunctionDecl {
                name: "max".to_string(),
                params: vec![
                    ("a".to_string(), LltsType::I32),
                    ("b".to_string(), LltsType::I32),
                ],
                ret_type: LltsType::I32,
                body: vec![Stmt::If {
                    condition: Expr::Binary {
                        op: BinOp::Gt,
                        lhs: Box::new(Expr::Var {
                            name: "a".to_string(),
                            ty: LltsType::I32,
                        }),
                        rhs: Box::new(Expr::Var {
                            name: "b".to_string(),
                            ty: LltsType::I32,
                        }),
                        ty: LltsType::I32,
                    },
                    then_body: vec![Stmt::Return(Some(Expr::Var {
                        name: "a".to_string(),
                        ty: LltsType::I32,
                    }))],
                    else_body: Some(vec![Stmt::Return(Some(Expr::Var {
                        name: "b".to_string(),
                        ty: LltsType::I32,
                    }))]),
                }],
            }],
        };

        codegen.compile(&program);
        assert!(codegen.module().verify().is_ok());
    }

    #[test]
    fn test_while_loop() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");

        // function countdown(n: i32): i32 {
        //   let result: i32 = 0;
        //   while (n > 0) { result = result + n; n = n - 1; }
        //   return result;
        // }
        let program = ProgramIR {
            structs: vec![],
            enums: vec![],
            functions: vec![FunctionDecl {
                name: "countdown".to_string(),
                params: vec![("n".to_string(), LltsType::I32)],
                ret_type: LltsType::I32,
                body: vec![
                    Stmt::VarDecl {
                        name: "result".to_string(),
                        ty: LltsType::I32,
                        init: Some(Expr::IntLit {
                            value: 0,
                            ty: LltsType::I32,
                        }),
                    },
                    Stmt::While {
                        condition: Expr::Binary {
                            op: BinOp::Gt,
                            lhs: Box::new(Expr::Var {
                                name: "n".to_string(),
                                ty: LltsType::I32,
                            }),
                            rhs: Box::new(Expr::IntLit {
                                value: 0,
                                ty: LltsType::I32,
                            }),
                            ty: LltsType::I32,
                        },
                        body: vec![
                            Stmt::Assign {
                                target: "result".to_string(),
                                value: Expr::Binary {
                                    op: BinOp::Add,
                                    lhs: Box::new(Expr::Var {
                                        name: "result".to_string(),
                                        ty: LltsType::I32,
                                    }),
                                    rhs: Box::new(Expr::Var {
                                        name: "n".to_string(),
                                        ty: LltsType::I32,
                                    }),
                                    ty: LltsType::I32,
                                },
                            },
                            Stmt::Assign {
                                target: "n".to_string(),
                                value: Expr::Binary {
                                    op: BinOp::Sub,
                                    lhs: Box::new(Expr::Var {
                                        name: "n".to_string(),
                                        ty: LltsType::I32,
                                    }),
                                    rhs: Box::new(Expr::IntLit {
                                        value: 1,
                                        ty: LltsType::I32,
                                    }),
                                    ty: LltsType::I32,
                                },
                            },
                        ],
                    },
                    Stmt::Return(Some(Expr::Var {
                        name: "result".to_string(),
                        ty: LltsType::I32,
                    })),
                ],
            }],
        };

        codegen.compile(&program);
        assert!(codegen.module().verify().is_ok());
    }

    #[test]
    fn test_print_hello_world() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");

        let program = ProgramIR {
            structs: vec![],
            enums: vec![],
            functions: vec![FunctionDecl {
                name: "main".to_string(),
                params: vec![],
                ret_type: LltsType::Void,
                body: vec![Stmt::Expr(Expr::Call {
                    callee: "print".to_string(),
                    args: vec![Expr::StringLit("Hello, World!".to_string())],
                    ret_type: LltsType::Void,
                })],
            }],
        };

        codegen.compile(&program);
        assert!(codegen.module().verify().is_ok());
    }

    #[test]
    fn test_struct_and_field_access() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");

        let point_type = LltsType::Struct {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), LltsType::F64),
                ("y".to_string(), LltsType::F64),
            ],
        };

        let program = ProgramIR {
            structs: vec![StructDecl {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), LltsType::F64),
                    ("y".to_string(), LltsType::F64),
                ],
            }],
            enums: vec![],
            functions: vec![FunctionDecl {
                name: "get_x".to_string(),
                params: vec![("p".to_string(), point_type.clone())],
                ret_type: LltsType::F64,
                body: vec![Stmt::Return(Some(Expr::FieldAccess {
                    object: Box::new(Expr::Var {
                        name: "p".to_string(),
                        ty: point_type.clone(),
                    }),
                    object_type: point_type.clone(),
                    field_index: 0,
                    field_type: LltsType::F64,
                }))],
            }],
        };

        codegen.compile(&program);
        assert!(codegen.module().verify().is_ok());
    }

    /// Test that destructured variables (emitted as flat VarDecl statements,
    /// not wrapped in Stmt::Block) remain accessible in the enclosing scope.
    /// Before the fix, destructuring lowered to Stmt::Block([VarDecl, VarDecl, ...]),
    /// and Block's save/restore of the variable map would erase those vars.
    #[test]
    fn test_destructured_vars_not_scoped_to_block() {
        let context = Context::create();
        let mut codegen = CodeGenerator::new(&context, "test");

        let point_type = LltsType::Struct {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), LltsType::F64),
                ("y".to_string(), LltsType::F64),
            ],
        };

        // Simulates: const p: Point = { x: 1.0, y: 2.0 };
        //            const { x, y } = p;
        //            return x + y;
        // After lowering, destructuring emits flat VarDecl stmts (not a Block).
        let program = ProgramIR {
            structs: vec![StructDecl {
                name: "Point".to_string(),
                fields: vec![
                    ("x".to_string(), LltsType::F64),
                    ("y".to_string(), LltsType::F64),
                ],
            }],
            enums: vec![],
            functions: vec![FunctionDecl {
                name: "destructure_test".to_string(),
                params: vec![("p".to_string(), point_type.clone())],
                ret_type: LltsType::F64,
                body: vec![
                    // __destructure_tmp_0 = p
                    Stmt::VarDecl {
                        name: "__destructure_tmp_0".to_string(),
                        ty: point_type.clone(),
                        init: Some(Expr::Var {
                            name: "p".to_string(),
                            ty: point_type.clone(),
                        }),
                    },
                    // x = __destructure_tmp_0.x
                    Stmt::VarDecl {
                        name: "x".to_string(),
                        ty: LltsType::F64,
                        init: Some(Expr::FieldAccess {
                            object: Box::new(Expr::Var {
                                name: "__destructure_tmp_0".to_string(),
                                ty: point_type.clone(),
                            }),
                            object_type: point_type.clone(),
                            field_index: 0,
                            field_type: LltsType::F64,
                        }),
                    },
                    // y = __destructure_tmp_0.y
                    Stmt::VarDecl {
                        name: "y".to_string(),
                        ty: LltsType::F64,
                        init: Some(Expr::FieldAccess {
                            object: Box::new(Expr::Var {
                                name: "__destructure_tmp_0".to_string(),
                                ty: point_type.clone(),
                            }),
                            object_type: point_type.clone(),
                            field_index: 1,
                            field_type: LltsType::F64,
                        }),
                    },
                    // return x + y  (uses destructured variables)
                    Stmt::Return(Some(Expr::Binary {
                        op: BinOp::Add,
                        lhs: Box::new(Expr::Var {
                            name: "x".to_string(),
                            ty: LltsType::F64,
                        }),
                        rhs: Box::new(Expr::Var {
                            name: "y".to_string(),
                            ty: LltsType::F64,
                        }),
                        ty: LltsType::F64,
                    })),
                ],
            }],
        };

        codegen.compile(&program);
        assert!(codegen.module().verify().is_ok());
    }
}
