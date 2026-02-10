use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};
use inkwell::IntPredicate;

use crate::types::{LltsType, TypeRegistry};

/// Type narrowing code generation.
///
/// Generates LLVM IR for TypeScript narrowing patterns:
/// - Discriminated union switch (e.g. `switch (shape.kind)`)
/// - instanceof checks (tag comparison)
/// - Null / Option<T> checks
/// - Type guard functions
pub struct NarrowingCodegen;

impl NarrowingCodegen {
    /// Build a switch on a discriminant field.
    ///
    /// Given a tagged union value, extract the integer tag (field 0) and emit
    /// an LLVM switch instruction branching to one basic block per variant.
    ///
    /// Returns the list of `(tag_value, basic_block)` pairs and the default
    /// (unreachable/exhaustiveness-failure) block so the caller can emit
    /// variant-specific code in each block.
    pub fn build_discriminant_switch<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        union_val: BasicValueEnum<'ctx>,
        variant_names: &[String],
    ) -> (Vec<(u32, BasicBlock<'ctx>)>, BasicBlock<'ctx>) {
        let i32_ty = context.i32_type();

        // Extract the tag (field 0 of the union struct).
        let tag = builder
            .build_extract_value(union_val.into_struct_value(), 0, "tag")
            .unwrap()
            .into_int_value();

        // Create a basic block for each variant + a default block.
        let mut cases: Vec<(u32, BasicBlock<'ctx>)> = Vec::new();
        let mut switch_cases: Vec<(IntValue<'ctx>, BasicBlock<'ctx>)> = Vec::new();

        for (i, name) in variant_names.iter().enumerate() {
            let bb = context.append_basic_block(function, &format!("case_{name}"));
            cases.push((i as u32, bb));
            switch_cases.push((i32_ty.const_int(i as u64, false), bb));
        }

        let default_bb = context.append_basic_block(function, "switch_default");

        // Build the switch.
        let _switch = builder.build_switch(tag, default_bb, &switch_cases).unwrap();

        (cases, default_bb)
    }

    /// Build an instanceof check: compare the union tag to a specific variant index.
    ///
    /// Returns an i1 value (true if the tag matches).
    pub fn build_instanceof_check<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        union_val: BasicValueEnum<'ctx>,
        expected_tag: u32,
    ) -> IntValue<'ctx> {
        let i32_ty = context.i32_type();

        let tag = builder
            .build_extract_value(union_val.into_struct_value(), 0, "tag")
            .unwrap()
            .into_int_value();
        let expected = i32_ty.const_int(expected_tag as u64, false);

        builder
            .build_int_compare(IntPredicate::EQ, tag, expected, "instanceof")
            .unwrap()
    }

    /// Build a null check for Option<T>.
    ///
    /// Option<T> is `{ i1, T }` where tag=0 means None, tag=1 means Some.
    /// Returns an i1 value: true if the option is Some (non-null).
    pub fn build_option_is_some<'ctx>(
        builder: &Builder<'ctx>,
        option_val: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        builder
            .build_extract_value(option_val.into_struct_value(), 0, "is_some")
            .unwrap()
            .into_int_value()
    }

    /// Build a null check for Option<T>.
    ///
    /// Returns an i1 value: true if the option is None (null).
    pub fn build_option_is_none<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        option_val: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        let is_some = Self::build_option_is_some(builder, option_val);
        let zero = context.bool_type().const_int(0, false);
        builder
            .build_int_compare(IntPredicate::EQ, is_some, zero, "is_none")
            .unwrap()
    }

    /// Unwrap an Option<T>, extracting the inner value.
    ///
    /// The caller must ensure the option is Some before calling this (via a
    /// null check + branch).
    pub fn build_option_unwrap<'ctx>(
        builder: &Builder<'ctx>,
        option_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        builder
            .build_extract_value(option_val.into_struct_value(), 1, "unwrapped")
            .unwrap()
    }

    /// Build an Option<T> with a Some value.
    pub fn build_option_some<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        inner_type: &LltsType,
        value: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let opt_ty = registry.option_type(inner_type);
        let opt_val = opt_ty.get_undef();
        let tag = context.bool_type().const_int(1, false);
        let opt_val = builder
            .build_insert_value(opt_val, tag, 0, "some_tag")
            .unwrap()
            .into_struct_value();
        let opt_val = builder
            .build_insert_value(opt_val, value, 1, "some_val")
            .unwrap()
            .into_struct_value();
        opt_val.into()
    }

    /// Build an Option<T> with a None value.
    pub fn build_option_none<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        inner_type: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        let opt_ty = registry.option_type(inner_type);
        let opt_val = opt_ty.get_undef();
        let tag = context.bool_type().const_int(0, false);
        let opt_val = builder
            .build_insert_value(opt_val, tag, 0, "none_tag")
            .unwrap()
            .into_struct_value();
        opt_val.into()
    }

    /// Build an if-let style narrowing for Option<T>.
    ///
    /// ```text
    /// if (value !== null) {
    ///   // value is T here
    /// } else {
    ///   // value is null here
    /// }
    /// ```
    ///
    /// Emits:
    /// - Check tag of Option
    /// - Branch: some_bb (narrowed to T), none_bb (null path)
    /// - merge_bb for continuation
    pub fn build_option_narrow<'ctx>(
        context: &'ctx Context,
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        option_val: BasicValueEnum<'ctx>,
        build_some: impl FnOnce(&Builder<'ctx>, BasicValueEnum<'ctx>),
        build_none: impl FnOnce(&Builder<'ctx>),
    ) -> BasicBlock<'ctx> {
        let is_some = Self::build_option_is_some(builder, option_val);

        let some_bb = context.append_basic_block(function, "some");
        let none_bb = context.append_basic_block(function, "none");
        let merge_bb = context.append_basic_block(function, "narrow_merge");

        builder
            .build_conditional_branch(is_some, some_bb, none_bb)
            .unwrap();

        // Some branch: extract inner value.
        builder.position_at_end(some_bb);
        let inner = Self::build_option_unwrap(builder, option_val);
        build_some(builder, inner);
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            builder.build_unconditional_branch(merge_bb).unwrap();
        }

        // None branch.
        builder.position_at_end(none_bb);
        build_none(builder);
        if builder.get_insert_block().unwrap().get_terminator().is_none() {
            builder.build_unconditional_branch(merge_bb).unwrap();
        }

        builder.position_at_end(merge_bb);
        merge_bb
    }

    /// Extract the payload of a tagged union for a specific variant.
    ///
    /// The union layout is `{ i32_tag, payload }`. After a switch/instanceof
    /// confirms the tag, we extract field 1 and bitcast it to the variant's
    /// actual type.
    pub fn build_union_extract<'ctx>(
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        union_val: BasicValueEnum<'ctx>,
        variant_type: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        // Extract field 1 (the payload slot).
        let payload = builder
            .build_extract_value(union_val.into_struct_value(), 1, "payload")
            .unwrap();
        // The payload might be stored as the largest variant type. If the
        // requested variant is smaller, we need a bitcast. For now, if the
        // types match, return directly. Otherwise, store to an alloca and load
        // as the correct type.
        let target_ty = registry.llvm_type(variant_type);
        if payload.get_type() == target_ty {
            payload
        } else {
            // Bitcast via alloca: store as payload type, load as target type.
            let alloca = builder
                .build_alloca(payload.get_type(), "union_cast")
                .unwrap();
            builder.build_store(alloca, payload).unwrap();
            builder.build_load(target_ty, alloca, "variant_val").unwrap()
        }
    }

    /// Build a tagged union value from a tag and a variant value.
    pub fn build_union_value<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        union_type: &LltsType,
        tag: u32,
        variant_value: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let union_llvm_ty = registry.llvm_type(union_type);
        let i32_ty = context.i32_type();

        // Store tag + variant value into the union struct.
        // We use an alloca since the variant value might be smaller than the
        // payload slot.
        let alloca = builder
            .build_alloca(union_llvm_ty, "union_build")
            .unwrap();

        // Store tag.
        let tag_ptr = builder
            .build_struct_gep(union_llvm_ty, alloca, 0, "tag_ptr")
            .unwrap();
        builder
            .build_store(tag_ptr, i32_ty.const_int(tag as u64, false))
            .unwrap();

        // Store variant value into the payload slot.
        let payload_ptr = builder
            .build_struct_gep(union_llvm_ty, alloca, 1, "payload_ptr")
            .unwrap();
        // Bitcast if necessary: store variant through a ptr cast.
        builder.build_store(payload_ptr, variant_value).unwrap();

        builder
            .build_load(union_llvm_ty, alloca, "union_val")
            .unwrap()
    }
}
