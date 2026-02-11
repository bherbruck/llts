use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicType;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};
use inkwell::{AddressSpace, IntPredicate};

use crate::memory::MemoryManager;
use crate::types::{LltsType, TypeRegistry};

/// Standard library method implementations for arrays and strings.
/// These are emitted inline as LLVM IR rather than as function calls.
pub struct StdlibCodegen;

impl StdlibCodegen {
    // ========================================================================
    // Array methods
    // ========================================================================

    /// `arr.push(elem)` — append element, grow if needed.
    /// Returns the array struct value with updated len (and possibly new ptr/cap).
    pub fn build_array_push<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        function: FunctionValue<'ctx>,
        arr_val: BasicValueEnum<'ctx>,
        elem_val: BasicValueEnum<'ctx>,
        elem_type: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        let i64_ty = context.i64_type();
        let arr = arr_val.into_struct_value();

        let data_ptr = builder.build_extract_value(arr, 0, "push_data").unwrap().into_pointer_value();
        let len = builder.build_extract_value(arr, 1, "push_len").unwrap().into_int_value();
        let cap = builder.build_extract_value(arr, 2, "push_cap").unwrap().into_int_value();

        // Check if we need to grow: len == cap
        let needs_grow = builder
            .build_int_compare(IntPredicate::EQ, len, cap, "needs_grow")
            .unwrap();

        let grow_bb = context.append_basic_block(function, "push_grow");
        let store_bb = context.append_basic_block(function, "push_store");

        builder.build_conditional_branch(needs_grow, grow_bb, store_bb).unwrap();

        // Grow: new_cap = cap * 2 (or 4 if cap == 0), realloc
        builder.position_at_end(grow_bb);
        let is_zero = builder
            .build_int_compare(IntPredicate::EQ, cap, i64_ty.const_int(0, false), "cap_zero")
            .unwrap();
        let doubled = builder.build_int_mul(cap, i64_ty.const_int(2, false), "doubled").unwrap();
        let new_cap = builder
            .build_select(is_zero, i64_ty.const_int(4, false), doubled, "new_cap")
            .unwrap()
            .into_int_value();

        let elem_llvm_ty = registry.llvm_type(elem_type);
        let elem_size_val = elem_llvm_ty.size_of().unwrap_or(i64_ty.const_int(8, false));
        let new_bytes = builder.build_int_mul(new_cap, elem_size_val, "new_bytes").unwrap();

        let realloc = memory.get_or_declare_realloc(module);
        let new_ptr = builder
            .build_call(realloc, &[data_ptr.into(), new_bytes.into()], "new_data")
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_pointer_value();

        builder.build_unconditional_branch(store_bb).unwrap();
        let grow_bb_end = builder.get_insert_block().unwrap();

        // Store element
        builder.position_at_end(store_bb);

        // Phi for data_ptr and cap
        let ptr_phi = builder.build_phi(context.ptr_type(AddressSpace::default()), "data_phi").unwrap();
        ptr_phi.add_incoming(&[
            (&data_ptr, grow_bb.get_previous_basic_block().unwrap_or(function.get_first_basic_block().unwrap())),
            (&new_ptr, grow_bb_end),
        ]);
        let final_ptr = ptr_phi.as_basic_value().into_pointer_value();

        let cap_phi = builder.build_phi(i64_ty, "cap_phi").unwrap();
        cap_phi.add_incoming(&[
            (&cap, grow_bb.get_previous_basic_block().unwrap_or(function.get_first_basic_block().unwrap())),
            (&new_cap, grow_bb_end),
        ]);
        let final_cap = cap_phi.as_basic_value().into_int_value();

        // Store element at data[len]
        let elem_llvm_ty = registry.llvm_type(elem_type);
        let elem_ptr = unsafe {
            builder.build_gep(elem_llvm_ty, final_ptr, &[len], "push_elem_ptr").unwrap()
        };
        builder.build_store(elem_ptr, elem_val).unwrap();

        // new_len = len + 1
        let new_len = builder.build_int_add(len, i64_ty.const_int(1, false), "new_len").unwrap();

        // Build updated array struct { ptr, len, cap }
        let arr_ty = registry.array_type(elem_type);
        let arr_val = arr_ty.get_undef();
        let arr_val = builder.build_insert_value(arr_val, final_ptr, 0, "arr_ptr").unwrap().into_struct_value();
        let arr_val = builder.build_insert_value(arr_val, new_len, 1, "arr_len").unwrap().into_struct_value();
        let arr_val = builder.build_insert_value(arr_val, final_cap, 2, "arr_cap").unwrap().into_struct_value();
        arr_val.into()
    }

    /// `arr.pop()` — remove and return last element.
    pub fn build_array_pop<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        _function: FunctionValue<'ctx>,
        arr_val: BasicValueEnum<'ctx>,
        elem_type: &LltsType,
    ) -> (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>) {
        let i64_ty = context.i64_type();
        let arr = arr_val.into_struct_value();

        let data_ptr = builder.build_extract_value(arr, 0, "pop_data").unwrap().into_pointer_value();
        let len = builder.build_extract_value(arr, 1, "pop_len").unwrap().into_int_value();
        let cap = builder.build_extract_value(arr, 2, "pop_cap").unwrap().into_int_value();

        // new_len = len - 1
        let new_len = builder.build_int_sub(len, i64_ty.const_int(1, false), "new_len").unwrap();

        // Load element at data[new_len]
        let elem_llvm_ty = registry.llvm_type(elem_type);
        let elem_ptr = unsafe {
            builder.build_gep(elem_llvm_ty, data_ptr, &[new_len], "pop_elem_ptr").unwrap()
        };
        let elem = builder.build_load(elem_llvm_ty, elem_ptr, "pop_elem").unwrap();

        // Build updated array struct
        let arr_ty = registry.array_type(elem_type);
        let arr_val = arr_ty.get_undef();
        let arr_val = builder.build_insert_value(arr_val, data_ptr, 0, "arr_ptr").unwrap().into_struct_value();
        let arr_val = builder.build_insert_value(arr_val, new_len, 1, "arr_len").unwrap().into_struct_value();
        let arr_val = builder.build_insert_value(arr_val, cap, 2, "arr_cap").unwrap().into_struct_value();

        (elem, arr_val.into())
    }

    /// `arr.indexOf(target)` — return index of first match, or -1.
    pub fn build_array_indexof<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        function: FunctionValue<'ctx>,
        arr_val: BasicValueEnum<'ctx>,
        target: BasicValueEnum<'ctx>,
        elem_type: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        let i64_ty = context.i64_type();
        let arr = arr_val.into_struct_value();

        let data_ptr = builder.build_extract_value(arr, 0, "idx_data").unwrap().into_pointer_value();
        let len = builder.build_extract_value(arr, 1, "idx_len").unwrap().into_int_value();

        let elem_llvm_ty = registry.llvm_type(elem_type);

        // Loop: i = 0; while (i < len) { if (data[i] == target) return i; i++; } return -1;
        let idx_alloca = builder.build_alloca(i64_ty, "idx_i").unwrap();
        builder.build_store(idx_alloca, i64_ty.const_int(0, false)).unwrap();

        let cond_bb = context.append_basic_block(function, "idx_cond");
        let body_bb = context.append_basic_block(function, "idx_body");
        let found_bb = context.append_basic_block(function, "idx_found");
        let next_bb = context.append_basic_block(function, "idx_next");
        let done_bb = context.append_basic_block(function, "idx_done");

        builder.build_unconditional_branch(cond_bb).unwrap();

        // Condition
        builder.position_at_end(cond_bb);
        let i = builder.build_load(i64_ty, idx_alloca, "i").unwrap().into_int_value();
        let cmp = builder.build_int_compare(IntPredicate::ULT, i, len, "idx_cmp").unwrap();
        builder.build_conditional_branch(cmp, body_bb, done_bb).unwrap();

        // Body: compare elements
        builder.position_at_end(body_bb);
        let elem_ptr = unsafe {
            builder.build_gep(elem_llvm_ty, data_ptr, &[i], "idx_elem_ptr").unwrap()
        };
        let elem = builder.build_load(elem_llvm_ty, elem_ptr, "idx_elem").unwrap();

        let eq = if TypeRegistry::is_integer(elem_type) || matches!(elem_type, LltsType::Bool) {
            builder.build_int_compare(IntPredicate::EQ, elem.into_int_value(), target.into_int_value(), "eq").unwrap()
        } else if TypeRegistry::is_float(elem_type) {
            builder.build_float_compare(inkwell::FloatPredicate::OEQ, elem.into_float_value(), target.into_float_value(), "eq").unwrap()
        } else {
            // Fallback for non-numeric types: just return false
            context.bool_type().const_int(0, false)
        };
        builder.build_conditional_branch(eq, found_bb, next_bb).unwrap();

        // Found
        builder.position_at_end(found_bb);
        builder.build_unconditional_branch(done_bb).unwrap();

        // Next
        builder.position_at_end(next_bb);
        let next_i = builder.build_int_add(i, i64_ty.const_int(1, false), "next_i").unwrap();
        builder.build_store(idx_alloca, next_i).unwrap();
        builder.build_unconditional_branch(cond_bb).unwrap();

        // Done: phi(-1 from cond, i from found)
        builder.position_at_end(done_bb);
        let phi = builder.build_phi(i64_ty, "indexof_result").unwrap();
        let neg_one = i64_ty.const_int(u64::MAX, true); // -1
        phi.add_incoming(&[(&neg_one, cond_bb), (&i, found_bb)]);
        phi.as_basic_value()
    }

    /// `arr.includes(target)` — return true if target is in array.
    pub fn build_array_includes<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        function: FunctionValue<'ctx>,
        arr_val: BasicValueEnum<'ctx>,
        target: BasicValueEnum<'ctx>,
        elem_type: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        let idx = Self::build_array_indexof(builder, context, registry, function, arr_val, target, elem_type);
        let i64_ty = context.i64_type();
        let neg_one = i64_ty.const_int(u64::MAX, true);
        let not_neg = builder
            .build_int_compare(IntPredicate::NE, idx.into_int_value(), neg_one, "includes")
            .unwrap();
        not_neg.into()
    }

    // ========================================================================
    // String methods
    // ========================================================================

    /// `str.charAt(i)` — return single-character string.
    pub fn build_string_charat<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        index: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let i64_ty = context.i64_type();
        let i8_ty = context.i8_type();
        let s = str_val.into_struct_value();
        let ptr = builder.build_extract_value(s, 0, "ca_ptr").unwrap().into_pointer_value();

        // GEP to the byte
        let byte_ptr = unsafe {
            builder.build_gep(i8_ty, ptr, &[index], "ca_byte_ptr").unwrap()
        };

        // Allocate 1 byte on heap and copy
        let one = i64_ty.const_int(1, false);
        let new_ptr = memory.build_heap_alloc(builder, module, one, "ca_alloc");
        let byte = builder.build_load(i8_ty, byte_ptr, "ca_byte").unwrap();
        builder.build_store(new_ptr, byte).unwrap();

        // Build { ptr, len=1 }
        let str_ty = registry.string_type();
        let str_val = str_ty.get_undef();
        let str_val = builder.build_insert_value(str_val, new_ptr, 0, "ca_str_ptr").unwrap().into_struct_value();
        let str_val = builder.build_insert_value(str_val, one, 1, "ca_str_len").unwrap().into_struct_value();
        str_val.into()
    }

    /// `str.charCodeAt(i)` — return byte value as i64.
    pub fn build_string_charcodeat<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        str_val: BasicValueEnum<'ctx>,
        index: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let i8_ty = context.i8_type();
        let i64_ty = context.i64_type();
        let s = str_val.into_struct_value();
        let ptr = builder.build_extract_value(s, 0, "cc_ptr").unwrap().into_pointer_value();

        let byte_ptr = unsafe {
            builder.build_gep(i8_ty, ptr, &[index], "cc_byte_ptr").unwrap()
        };
        let byte = builder.build_load(i8_ty, byte_ptr, "cc_byte").unwrap().into_int_value();
        let wide = builder.build_int_z_extend(byte, i64_ty, "cc_wide").unwrap();
        wide.into()
    }

    /// `str.indexOf(target)` — return index of first occurrence, or -1.
    pub fn build_string_indexof<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        target_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let i64_ty = context.i64_type();
        let s = str_val.into_struct_value();
        let t = target_val.into_struct_value();

        let s_ptr = builder.build_extract_value(s, 0, "si_s_ptr").unwrap().into_pointer_value();
        let s_len = builder.build_extract_value(s, 1, "si_s_len").unwrap().into_int_value();
        let t_ptr = builder.build_extract_value(t, 0, "si_t_ptr").unwrap().into_pointer_value();
        let t_len = builder.build_extract_value(t, 1, "si_t_len").unwrap().into_int_value();

        // If target is empty, return 0
        // If target is longer than str, return -1
        // Otherwise loop: for i in 0..=(s_len - t_len), memcmp(s_ptr+i, t_ptr, t_len)==0 -> return i

        let memcmp = Self::get_or_declare_memcmp(context, module);

        // search_len = s_len - t_len + 1
        let search_len = builder.build_int_sub(s_len, t_len, "search_sub").unwrap();
        let search_len = builder.build_int_add(search_len, i64_ty.const_int(1, false), "search_len").unwrap();

        let idx_alloca = builder.build_alloca(i64_ty, "si_i").unwrap();
        builder.build_store(idx_alloca, i64_ty.const_int(0, false)).unwrap();

        let cond_bb = context.append_basic_block(function, "si_cond");
        let body_bb = context.append_basic_block(function, "si_body");
        let found_bb = context.append_basic_block(function, "si_found");
        let next_bb = context.append_basic_block(function, "si_next");
        let done_bb = context.append_basic_block(function, "si_done");

        builder.build_unconditional_branch(cond_bb).unwrap();

        builder.position_at_end(cond_bb);
        let i = builder.build_load(i64_ty, idx_alloca, "i").unwrap().into_int_value();
        let cmp = builder.build_int_compare(IntPredicate::ULT, i, search_len, "si_cmp").unwrap();
        builder.build_conditional_branch(cmp, body_bb, done_bb).unwrap();

        builder.position_at_end(body_bb);
        let i8_ty = context.i8_type();
        let offset_ptr = unsafe {
            builder.build_gep(i8_ty, s_ptr, &[i], "si_offset").unwrap()
        };
        let cmp_result = builder
            .build_call(memcmp, &[offset_ptr.into(), t_ptr.into(), t_len.into()], "si_memcmp")
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();
        let is_eq = builder
            .build_int_compare(IntPredicate::EQ, cmp_result, context.i32_type().const_int(0, false), "si_eq")
            .unwrap();
        builder.build_conditional_branch(is_eq, found_bb, next_bb).unwrap();

        builder.position_at_end(found_bb);
        builder.build_unconditional_branch(done_bb).unwrap();

        builder.position_at_end(next_bb);
        let next_i = builder.build_int_add(i, i64_ty.const_int(1, false), "si_next_i").unwrap();
        builder.build_store(idx_alloca, next_i).unwrap();
        builder.build_unconditional_branch(cond_bb).unwrap();

        builder.position_at_end(done_bb);
        let phi = builder.build_phi(i64_ty, "si_result").unwrap();
        let neg_one = i64_ty.const_int(u64::MAX, true);
        phi.add_incoming(&[(&neg_one, cond_bb), (&i, found_bb)]);
        phi.as_basic_value()
    }

    /// `str.includes(target)` — return true if target is found.
    pub fn build_string_includes<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        target_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let idx = Self::build_string_indexof(builder, context, module, function, str_val, target_val);
        let i64_ty = context.i64_type();
        let neg_one = i64_ty.const_int(u64::MAX, true);
        let found = builder
            .build_int_compare(IntPredicate::NE, idx.into_int_value(), neg_one, "includes")
            .unwrap();
        found.into()
    }

    /// `str.slice(start, end)` — return substring.
    pub fn build_string_slice<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        start: IntValue<'ctx>,
        end: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let i8_ty = context.i8_type();
        let s = str_val.into_struct_value();
        let ptr = builder.build_extract_value(s, 0, "sl_ptr").unwrap().into_pointer_value();

        let new_len = builder.build_int_sub(end, start, "sl_len").unwrap();

        let src = unsafe {
            builder.build_gep(i8_ty, ptr, &[start], "sl_src").unwrap()
        };

        // Allocate and copy
        let result = memory.build_string_alloc(builder, module, registry, src, new_len);
        result
    }

    /// `str.toUpperCase()` — return new string with all bytes uppercased.
    pub fn build_string_touppercase<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        Self::build_string_case_transform(builder, context, module, registry, memory, function, str_val, true)
    }

    /// `str.toLowerCase()` — return new string with all bytes lowercased.
    pub fn build_string_tolowercase<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        Self::build_string_case_transform(builder, context, module, registry, memory, function, str_val, false)
    }

    fn build_string_case_transform<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        to_upper: bool,
    ) -> BasicValueEnum<'ctx> {
        let i8_ty = context.i8_type();
        let i64_ty = context.i64_type();
        let s = str_val.into_struct_value();
        let ptr = builder.build_extract_value(s, 0, "tc_ptr").unwrap().into_pointer_value();
        let len = builder.build_extract_value(s, 1, "tc_len").unwrap().into_int_value();

        // Allocate new buffer
        let new_ptr = memory.build_heap_alloc(builder, module, len, "tc_alloc");

        // Loop through each byte and transform
        let idx_alloca = builder.build_alloca(i64_ty, "tc_i").unwrap();
        builder.build_store(idx_alloca, i64_ty.const_int(0, false)).unwrap();

        let cond_bb = context.append_basic_block(function, "tc_cond");
        let body_bb = context.append_basic_block(function, "tc_body");
        let done_bb = context.append_basic_block(function, "tc_done");

        builder.build_unconditional_branch(cond_bb).unwrap();

        builder.position_at_end(cond_bb);
        let i = builder.build_load(i64_ty, idx_alloca, "i").unwrap().into_int_value();
        let cmp = builder.build_int_compare(IntPredicate::ULT, i, len, "tc_cmp").unwrap();
        builder.build_conditional_branch(cmp, body_bb, done_bb).unwrap();

        builder.position_at_end(body_bb);
        let src_byte_ptr = unsafe { builder.build_gep(i8_ty, ptr, &[i], "tc_src").unwrap() };
        let byte = builder.build_load(i8_ty, src_byte_ptr, "tc_byte").unwrap().into_int_value();

        let (range_start, range_end, offset) = if to_upper {
            // 'a' (97) to 'z' (122), subtract 32
            (97u64, 122u64, 32u64)
        } else {
            // 'A' (65) to 'Z' (90), add 32
            (65u64, 90u64, 32u64)
        };

        let in_range_lo = builder
            .build_int_compare(IntPredicate::UGE, byte, i8_ty.const_int(range_start, false), "in_lo")
            .unwrap();
        let in_range_hi = builder
            .build_int_compare(IntPredicate::ULE, byte, i8_ty.const_int(range_end, false), "in_hi")
            .unwrap();
        let in_range = builder.build_and(in_range_lo, in_range_hi, "in_range").unwrap();

        let transformed = if to_upper {
            builder.build_int_sub(byte, i8_ty.const_int(offset, false), "upper").unwrap()
        } else {
            builder.build_int_add(byte, i8_ty.const_int(offset, false), "lower").unwrap()
        };
        let result_byte = builder.build_select(in_range, transformed, byte, "tc_result").unwrap();

        let dst_byte_ptr = unsafe { builder.build_gep(i8_ty, new_ptr, &[i], "tc_dst").unwrap() };
        builder.build_store(dst_byte_ptr, result_byte).unwrap();

        let next_i = builder.build_int_add(i, i64_ty.const_int(1, false), "tc_next_i").unwrap();
        builder.build_store(idx_alloca, next_i).unwrap();
        builder.build_unconditional_branch(cond_bb).unwrap();

        builder.position_at_end(done_bb);

        let str_ty = registry.string_type();
        let result = str_ty.get_undef();
        let result = builder.build_insert_value(result, new_ptr, 0, "tc_str_ptr").unwrap().into_struct_value();
        let result = builder.build_insert_value(result, len, 1, "tc_str_len").unwrap().into_struct_value();
        result.into()
    }

    /// `str.trim()` — remove leading and trailing whitespace.
    pub fn build_string_trim<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        memory: &mut MemoryManager<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let i8_ty = context.i8_type();
        let i64_ty = context.i64_type();
        let s = str_val.into_struct_value();
        let ptr = builder.build_extract_value(s, 0, "tr_ptr").unwrap().into_pointer_value();
        let len = builder.build_extract_value(s, 1, "tr_len").unwrap().into_int_value();

        // Find start: skip whitespace (space=32, tab=9, newline=10, carriage return=13)
        let start_alloca = builder.build_alloca(i64_ty, "tr_start").unwrap();
        builder.build_store(start_alloca, i64_ty.const_int(0, false)).unwrap();

        let start_cond = context.append_basic_block(function, "tr_start_cond");
        let start_body = context.append_basic_block(function, "tr_start_body");
        let start_done = context.append_basic_block(function, "tr_start_done");

        builder.build_unconditional_branch(start_cond).unwrap();

        builder.position_at_end(start_cond);
        let si = builder.build_load(i64_ty, start_alloca, "si").unwrap().into_int_value();
        let in_bounds = builder.build_int_compare(IntPredicate::ULT, si, len, "si_bounds").unwrap();
        builder.build_conditional_branch(in_bounds, start_body, start_done).unwrap();

        builder.position_at_end(start_body);
        let byte_ptr = unsafe { builder.build_gep(i8_ty, ptr, &[si], "tr_byte_ptr").unwrap() };
        let byte = builder.build_load(i8_ty, byte_ptr, "tr_byte").unwrap().into_int_value();
        let is_space = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(32, false), "is_sp").unwrap();
        let is_tab = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(9, false), "is_tab").unwrap();
        let is_nl = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(10, false), "is_nl").unwrap();
        let is_cr = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(13, false), "is_cr").unwrap();
        let is_ws = builder.build_or(is_space, is_tab, "ws1").unwrap();
        let is_ws = builder.build_or(is_ws, is_nl, "ws2").unwrap();
        let is_ws = builder.build_or(is_ws, is_cr, "ws3").unwrap();
        let next_si = builder.build_int_add(si, i64_ty.const_int(1, false), "next_si").unwrap();
        builder.build_store(start_alloca, next_si).unwrap();
        builder.build_conditional_branch(is_ws, start_cond, start_done).unwrap();

        builder.position_at_end(start_done);
        let start_phi = builder.build_phi(i64_ty, "trim_start").unwrap();
        start_phi.add_incoming(&[(&si, start_cond), (&si, start_body)]);
        let trim_start = start_phi.as_basic_value().into_int_value();

        // Find end: scan from end backwards
        let end_alloca = builder.build_alloca(i64_ty, "tr_end").unwrap();
        builder.build_store(end_alloca, len).unwrap();

        let end_cond = context.append_basic_block(function, "tr_end_cond");
        let end_body = context.append_basic_block(function, "tr_end_body");
        let end_done = context.append_basic_block(function, "tr_end_done");

        builder.build_unconditional_branch(end_cond).unwrap();

        builder.position_at_end(end_cond);
        let ei = builder.build_load(i64_ty, end_alloca, "ei").unwrap().into_int_value();
        let gt_start = builder.build_int_compare(IntPredicate::UGT, ei, trim_start, "ei_gt").unwrap();
        builder.build_conditional_branch(gt_start, end_body, end_done).unwrap();

        builder.position_at_end(end_body);
        let prev_ei = builder.build_int_sub(ei, i64_ty.const_int(1, false), "prev_ei").unwrap();
        let byte_ptr = unsafe { builder.build_gep(i8_ty, ptr, &[prev_ei], "tr_end_byte_ptr").unwrap() };
        let byte = builder.build_load(i8_ty, byte_ptr, "tr_end_byte").unwrap().into_int_value();
        let is_space = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(32, false), "end_sp").unwrap();
        let is_tab = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(9, false), "end_tab").unwrap();
        let is_nl = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(10, false), "end_nl").unwrap();
        let is_cr = builder.build_int_compare(IntPredicate::EQ, byte, i8_ty.const_int(13, false), "end_cr").unwrap();
        let is_ws = builder.build_or(is_space, is_tab, "end_ws1").unwrap();
        let is_ws = builder.build_or(is_ws, is_nl, "end_ws2").unwrap();
        let is_ws = builder.build_or(is_ws, is_cr, "end_ws3").unwrap();
        builder.build_store(end_alloca, prev_ei).unwrap();
        builder.build_conditional_branch(is_ws, end_cond, end_done).unwrap();

        builder.position_at_end(end_done);
        let end_phi = builder.build_phi(i64_ty, "trim_end").unwrap();
        end_phi.add_incoming(&[(&ei, end_cond), (&ei, end_body)]);
        let trim_end = end_phi.as_basic_value().into_int_value();

        // Slice from trim_start to trim_end
        Self::build_string_slice(builder, context, module, registry, memory, str_val, trim_start, trim_end)
    }

    /// `str.startsWith(prefix)` — check if string starts with prefix.
    pub fn build_string_startswith<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        prefix_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let s = str_val.into_struct_value();
        let p = prefix_val.into_struct_value();

        let s_ptr = builder.build_extract_value(s, 0, "sw_s_ptr").unwrap().into_pointer_value();
        let s_len = builder.build_extract_value(s, 1, "sw_s_len").unwrap().into_int_value();
        let p_ptr = builder.build_extract_value(p, 0, "sw_p_ptr").unwrap().into_pointer_value();
        let p_len = builder.build_extract_value(p, 1, "sw_p_len").unwrap().into_int_value();

        // If prefix is longer than string, return false
        let long_enough = builder
            .build_int_compare(IntPredicate::UGE, s_len, p_len, "sw_long")
            .unwrap();

        let cmp_bb = context.append_basic_block(function, "sw_cmp");
        let done_bb = context.append_basic_block(function, "sw_done");

        builder.build_conditional_branch(long_enough, cmp_bb, done_bb).unwrap();

        builder.position_at_end(cmp_bb);
        let memcmp = Self::get_or_declare_memcmp(context, module);
        let cmp_result = builder
            .build_call(memcmp, &[s_ptr.into(), p_ptr.into(), p_len.into()], "sw_memcmp")
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();
        let is_eq = builder
            .build_int_compare(IntPredicate::EQ, cmp_result, context.i32_type().const_int(0, false), "sw_eq")
            .unwrap();
        builder.build_unconditional_branch(done_bb).unwrap();

        builder.position_at_end(done_bb);
        let phi = builder.build_phi(context.bool_type(), "startswith").unwrap();
        let false_val = context.bool_type().const_int(0, false);
        let entry_bb = cmp_bb.get_previous_basic_block().unwrap_or(function.get_first_basic_block().unwrap());
        phi.add_incoming(&[(&false_val, entry_bb), (&is_eq, cmp_bb)]);
        phi.as_basic_value()
    }

    /// `str.endsWith(suffix)` — check if string ends with suffix.
    pub fn build_string_endswith<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        str_val: BasicValueEnum<'ctx>,
        suffix_val: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let i8_ty = context.i8_type();
        let s = str_val.into_struct_value();
        let x = suffix_val.into_struct_value();

        let s_ptr = builder.build_extract_value(s, 0, "ew_s_ptr").unwrap().into_pointer_value();
        let s_len = builder.build_extract_value(s, 1, "ew_s_len").unwrap().into_int_value();
        let x_ptr = builder.build_extract_value(x, 0, "ew_x_ptr").unwrap().into_pointer_value();
        let x_len = builder.build_extract_value(x, 1, "ew_x_len").unwrap().into_int_value();

        let long_enough = builder
            .build_int_compare(IntPredicate::UGE, s_len, x_len, "ew_long")
            .unwrap();

        let cmp_bb = context.append_basic_block(function, "ew_cmp");
        let done_bb = context.append_basic_block(function, "ew_done");

        builder.build_conditional_branch(long_enough, cmp_bb, done_bb).unwrap();

        builder.position_at_end(cmp_bb);
        let offset = builder.build_int_sub(s_len, x_len, "ew_offset").unwrap();
        let cmp_ptr = unsafe {
            builder.build_gep(i8_ty, s_ptr, &[offset], "ew_cmp_ptr").unwrap()
        };
        let memcmp = Self::get_or_declare_memcmp(context, module);
        let cmp_result = builder
            .build_call(memcmp, &[cmp_ptr.into(), x_ptr.into(), x_len.into()], "ew_memcmp")
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();
        let is_eq = builder
            .build_int_compare(IntPredicate::EQ, cmp_result, context.i32_type().const_int(0, false), "ew_eq")
            .unwrap();
        builder.build_unconditional_branch(done_bb).unwrap();

        builder.position_at_end(done_bb);
        let phi = builder.build_phi(context.bool_type(), "endswith").unwrap();
        let false_val = context.bool_type().const_int(0, false);
        let entry_bb = cmp_bb.get_previous_basic_block().unwrap_or(function.get_first_basic_block().unwrap());
        phi.add_incoming(&[(&false_val, entry_bb), (&is_eq, cmp_bb)]);
        phi.as_basic_value()
    }

    // ---- Helpers ----

    fn get_or_declare_memcmp<'ctx>(
        context: &'ctx Context,
        module: &Module<'ctx>,
    ) -> FunctionValue<'ctx> {
        if let Some(f) = module.get_function("memcmp") {
            return f;
        }
        let ptr_ty = context.ptr_type(AddressSpace::default());
        let i32_ty = context.i32_type();
        let i64_ty = context.i64_type();
        let fn_type = i32_ty.fn_type(&[ptr_ty.into(), ptr_ty.into(), i64_ty.into()], false);
        module.add_function("memcmp", fn_type, None)
    }
}
