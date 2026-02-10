use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::IntPredicate;

use crate::types::{LltsType, TypeRegistry};

/// Statement code generation utilities.
///
/// Produces LLVM IR for variable declarations, assignments, if/else,
/// while/for loops, returns, and block statements (scope management).
pub struct StmtCodegen;

impl StmtCodegen {
    // ---- Variable declarations ----

    /// Emit a local variable declaration: `alloca` in the entry block + optional
    /// `store` of the initializer.
    ///
    /// Returns the alloca pointer for use by subsequent loads/stores.
    pub fn build_var_decl<'ctx>(
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        ty: &LltsType,
        name: &str,
        init: Option<BasicValueEnum<'ctx>>,
    ) -> PointerValue<'ctx> {
        let llvm_ty = registry.llvm_type(ty);
        let alloca = builder.build_alloca(llvm_ty, name).unwrap();

        if let Some(val) = init {
            builder.build_store(alloca, val).unwrap();
        }

        alloca
    }

    /// Emit an assignment: `store` a value into an existing alloca.
    pub fn build_assignment<'ctx>(
        builder: &Builder<'ctx>,
        ptr: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) {
        builder.build_store(ptr, value).unwrap();
    }

    // ---- Control flow: if/else ----

    /// Emit an if/else structure. Returns the merge basic block so the caller
    /// can continue emitting after the branch.
    ///
    /// `build_then` and `build_else` are callbacks that emit the body of each
    /// branch. They receive the builder positioned at the start of their block.
    pub fn build_if_else<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        condition: IntValue<'ctx>,
        build_then: impl FnOnce(&Builder<'ctx>),
        build_else: Option<impl FnOnce(&Builder<'ctx>)>,
    ) -> BasicBlock<'ctx> {
        let then_bb = context.append_basic_block(function, "then");
        let else_bb = context.append_basic_block(function, "else");
        let merge_bb = context.append_basic_block(function, "merge");

        builder
            .build_conditional_branch(condition, then_bb, else_bb)
            .unwrap();

        // Then branch.
        builder.position_at_end(then_bb);
        build_then(builder);
        // Only branch to merge if the then block is not already terminated.
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            builder.build_unconditional_branch(merge_bb).unwrap();
        }

        // Else branch.
        builder.position_at_end(else_bb);
        if let Some(build_el) = build_else {
            build_el(builder);
        }
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            builder.build_unconditional_branch(merge_bb).unwrap();
        }

        // Continue at merge.
        builder.position_at_end(merge_bb);
        merge_bb
    }

    // ---- Control flow: while loop ----

    /// Emit a while loop.
    ///
    /// ```text
    /// loop_cond:
    ///   %cond = <build_condition>
    ///   br %cond, loop_body, loop_end
    /// loop_body:
    ///   <build_body>
    ///   br loop_cond
    /// loop_end:
    /// ```
    ///
    /// Returns `(loop_cond_bb, loop_end_bb)` so the caller can handle
    /// `break` (branch to loop_end) and `continue` (branch to loop_cond).
    pub fn build_while_loop<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        build_condition: impl FnOnce(&Builder<'ctx>) -> IntValue<'ctx>,
        build_body: impl FnOnce(&Builder<'ctx>),
    ) -> (BasicBlock<'ctx>, BasicBlock<'ctx>) {
        let cond_bb = context.append_basic_block(function, "loop_cond");
        let body_bb = context.append_basic_block(function, "loop_body");
        let end_bb = context.append_basic_block(function, "loop_end");

        // Branch to condition check.
        builder.build_unconditional_branch(cond_bb).unwrap();

        // Condition.
        builder.position_at_end(cond_bb);
        let cond = build_condition(builder);
        builder
            .build_conditional_branch(cond, body_bb, end_bb)
            .unwrap();

        // Body.
        builder.position_at_end(body_bb);
        build_body(builder);
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            builder.build_unconditional_branch(cond_bb).unwrap();
        }

        // Continue after loop.
        builder.position_at_end(end_bb);
        (cond_bb, end_bb)
    }

    // ---- Control flow: for loop ----

    /// Emit a C-style for loop: `for (init; cond; update) { body }`.
    ///
    /// Returns `(cond_bb, end_bb)` for break/continue support.
    pub fn build_for_loop<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        build_init: impl FnOnce(&Builder<'ctx>),
        build_condition: impl FnOnce(&Builder<'ctx>) -> IntValue<'ctx>,
        build_update: impl FnOnce(&Builder<'ctx>),
        build_body: impl FnOnce(&Builder<'ctx>),
    ) -> (BasicBlock<'ctx>, BasicBlock<'ctx>) {
        // Init.
        build_init(builder);

        let cond_bb = context.append_basic_block(function, "for_cond");
        let body_bb = context.append_basic_block(function, "for_body");
        let update_bb = context.append_basic_block(function, "for_update");
        let end_bb = context.append_basic_block(function, "for_end");

        builder.build_unconditional_branch(cond_bb).unwrap();

        // Condition.
        builder.position_at_end(cond_bb);
        let cond = build_condition(builder);
        builder
            .build_conditional_branch(cond, body_bb, end_bb)
            .unwrap();

        // Body.
        builder.position_at_end(body_bb);
        build_body(builder);
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            builder.build_unconditional_branch(update_bb).unwrap();
        }

        // Update.
        builder.position_at_end(update_bb);
        build_update(builder);
        builder.build_unconditional_branch(cond_bb).unwrap();

        // End.
        builder.position_at_end(end_bb);
        (cond_bb, end_bb)
    }

    // ---- for...of on arrays ----

    /// Emit `for (const x of arr) { body }` as an index-based loop.
    ///
    /// Desugars to:
    /// ```text
    /// let _i = 0;
    /// while (_i < arr.len) {
    ///   const x = arr[_i];
    ///   <body>
    ///   _i += 1;
    /// }
    /// ```
    pub fn build_for_of_array<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        array_val: BasicValueEnum<'ctx>,
        elem_type: &LltsType,
        registry: &mut TypeRegistry<'ctx>,
        build_body: impl FnOnce(&Builder<'ctx>, BasicValueEnum<'ctx>),
    ) -> (BasicBlock<'ctx>, BasicBlock<'ctx>) {
        let i64_ty = context.i64_type();
        let elem_llvm_ty = registry.llvm_type(elem_type);

        // Extract array ptr and len.
        let arr = array_val.into_struct_value();
        let data_ptr = builder
            .build_extract_value(arr, 0, "forof_data")
            .unwrap()
            .into_pointer_value();
        let len = builder
            .build_extract_value(arr, 1, "forof_len")
            .unwrap()
            .into_int_value();

        // Index variable.
        let idx_alloca = builder.build_alloca(i64_ty, "forof_i").unwrap();
        builder
            .build_store(idx_alloca, i64_ty.const_int(0, false))
            .unwrap();

        let cond_bb = context.append_basic_block(function, "forof_cond");
        let body_bb = context.append_basic_block(function, "forof_body");
        let end_bb = context.append_basic_block(function, "forof_end");

        builder.build_unconditional_branch(cond_bb).unwrap();

        // Condition: _i < len.
        builder.position_at_end(cond_bb);
        let idx = builder
            .build_load(i64_ty, idx_alloca, "i")
            .unwrap()
            .into_int_value();
        let cond = builder
            .build_int_compare(IntPredicate::ULT, idx, len, "forof_cmp")
            .unwrap();
        builder
            .build_conditional_branch(cond, body_bb, end_bb)
            .unwrap();

        // Body: load element, call body, increment index.
        builder.position_at_end(body_bb);
        let elem_ptr = unsafe {
            builder
                .build_gep(elem_llvm_ty, data_ptr, &[idx], "elem_ptr")
                .unwrap()
        };
        let elem = builder
            .build_load(elem_llvm_ty, elem_ptr, "elem")
            .unwrap();
        build_body(builder, elem);

        // _i += 1.
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            let one = i64_ty.const_int(1, false);
            let next_idx = builder.build_int_add(idx, one, "next_i").unwrap();
            builder.build_store(idx_alloca, next_idx).unwrap();
            builder.build_unconditional_branch(cond_bb).unwrap();
        }

        builder.position_at_end(end_bb);
        (cond_bb, end_bb)
    }

    // ---- Return ----

    /// Emit a return statement.
    pub fn build_return<'ctx>(
        builder: &Builder<'ctx>,
        value: Option<&dyn inkwell::values::BasicValue<'ctx>>,
    ) {
        builder.build_return(value).unwrap();
    }
}
