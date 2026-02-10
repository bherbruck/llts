use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, FloatValue, FunctionValue, IntValue, PointerValue};
use inkwell::{FloatPredicate, IntPredicate};
use crate::types::{LltsType, TypeRegistry};

/// Binary operator kinds supported by the codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    LogicalAnd,
    LogicalOr,
}

/// Logical operator kinds (short-circuit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

/// Unary operator kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

/// Expression code generation utilities.
///
/// These are free functions rather than methods on `CodeGenerator` to keep the
/// file focused and avoid borrowing conflicts. The caller supplies the builder,
/// module, and registry.
pub struct ExprCodegen;

impl ExprCodegen {
    // ---- Literals ----

    pub fn const_i32(context: &Context, value: i64) -> BasicValueEnum<'_> {
        context.i32_type().const_int(value as u64, value < 0).into()
    }

    pub fn const_i64(context: &Context, value: i64) -> BasicValueEnum<'_> {
        context.i64_type().const_int(value as u64, value < 0).into()
    }

    pub fn const_f64(context: &Context, value: f64) -> BasicValueEnum<'_> {
        context.f64_type().const_float(value).into()
    }

    pub fn const_f32(context: &Context, value: f64) -> BasicValueEnum<'_> {
        context.f32_type().const_float(value).into()
    }

    pub fn const_bool(context: &Context, value: bool) -> BasicValueEnum<'_> {
        context
            .bool_type()
            .const_int(if value { 1 } else { 0 }, false)
            .into()
    }

    /// Create a string literal as a global constant and return { ptr, len }.
    pub fn const_string<'ctx>(
        builder: &Builder<'ctx>,
        _module: &Module<'ctx>,
        context: &'ctx Context,
        registry: &TypeRegistry<'ctx>,
        value: &str,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        let global = builder
            .build_global_string_ptr(value, name)
            .unwrap();
        let ptr = global.as_pointer_value();
        let len = context.i64_type().const_int(value.len() as u64, false);

        let str_ty = registry.string_type();
        let str_val = str_ty.get_undef();
        let str_val = builder
            .build_insert_value(str_val, ptr, 0, "str_ptr")
            .unwrap()
            .into_struct_value();
        let str_val = builder
            .build_insert_value(str_val, len, 1, "str_len")
            .unwrap()
            .into_struct_value();
        str_val.into()
    }

    // ---- Binary operations ----

    /// Emit a binary operation on two values of the same type.
    pub fn build_binary<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        op: BinOp,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
        ty: &LltsType,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        if TypeRegistry::is_integer(ty) {
            let signed = TypeRegistry::is_signed(ty);
            let l = lhs.into_int_value();
            let r = rhs.into_int_value();
            Self::build_int_binary(builder, context, op, l, r, signed, name).into()
        } else if TypeRegistry::is_float(ty) {
            let l = lhs.into_float_value();
            let r = rhs.into_float_value();
            Self::build_float_binary(builder, context, op, l, r, name)
        } else {
            panic!("build_binary: unsupported type {ty:?}")
        }
    }

    fn build_int_binary<'ctx>(
        builder: &Builder<'ctx>,
        _context: &'ctx Context,
        op: BinOp,
        lhs: IntValue<'ctx>,
        rhs: IntValue<'ctx>,
        signed: bool,
        name: &str,
    ) -> IntValue<'ctx> {
        match op {
            BinOp::Add => builder.build_int_add(lhs, rhs, name).unwrap(),
            BinOp::Sub => builder.build_int_sub(lhs, rhs, name).unwrap(),
            BinOp::Mul => builder.build_int_mul(lhs, rhs, name).unwrap(),
            BinOp::Div => {
                if signed {
                    builder.build_int_signed_div(lhs, rhs, name).unwrap()
                } else {
                    builder.build_int_unsigned_div(lhs, rhs, name).unwrap()
                }
            }
            BinOp::Rem => {
                if signed {
                    builder.build_int_signed_rem(lhs, rhs, name).unwrap()
                } else {
                    builder.build_int_unsigned_rem(lhs, rhs, name).unwrap()
                }
            }
            BinOp::BitAnd => builder.build_and(lhs, rhs, name).unwrap(),
            BinOp::BitOr => builder.build_or(lhs, rhs, name).unwrap(),
            BinOp::BitXor => builder.build_xor(lhs, rhs, name).unwrap(),
            BinOp::Shl => builder.build_left_shift(lhs, rhs, name).unwrap(),
            BinOp::Shr => builder.build_right_shift(lhs, rhs, signed, name).unwrap(),
            BinOp::Eq => builder
                .build_int_compare(IntPredicate::EQ, lhs, rhs, name)
                .unwrap(),
            BinOp::Ne => builder
                .build_int_compare(IntPredicate::NE, lhs, rhs, name)
                .unwrap(),
            BinOp::Lt => {
                let pred = if signed { IntPredicate::SLT } else { IntPredicate::ULT };
                builder.build_int_compare(pred, lhs, rhs, name).unwrap()
            }
            BinOp::Le => {
                let pred = if signed { IntPredicate::SLE } else { IntPredicate::ULE };
                builder.build_int_compare(pred, lhs, rhs, name).unwrap()
            }
            BinOp::Gt => {
                let pred = if signed { IntPredicate::SGT } else { IntPredicate::UGT };
                builder.build_int_compare(pred, lhs, rhs, name).unwrap()
            }
            BinOp::Ge => {
                let pred = if signed { IntPredicate::SGE } else { IntPredicate::UGE };
                builder.build_int_compare(pred, lhs, rhs, name).unwrap()
            }
            BinOp::LogicalAnd => builder.build_and(lhs, rhs, name).unwrap(),
            BinOp::LogicalOr => builder.build_or(lhs, rhs, name).unwrap(),
        }
    }

    fn build_float_binary<'ctx>(
        builder: &Builder<'ctx>,
        _context: &'ctx Context,
        op: BinOp,
        lhs: FloatValue<'ctx>,
        rhs: FloatValue<'ctx>,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        match op {
            BinOp::Add => builder.build_float_add(lhs, rhs, name).unwrap().into(),
            BinOp::Sub => builder.build_float_sub(lhs, rhs, name).unwrap().into(),
            BinOp::Mul => builder.build_float_mul(lhs, rhs, name).unwrap().into(),
            BinOp::Div => builder.build_float_div(lhs, rhs, name).unwrap().into(),
            BinOp::Rem => builder.build_float_rem(lhs, rhs, name).unwrap().into(),
            BinOp::Eq => builder
                .build_float_compare(FloatPredicate::OEQ, lhs, rhs, name)
                .unwrap()
                .into(),
            BinOp::Ne => builder
                .build_float_compare(FloatPredicate::ONE, lhs, rhs, name)
                .unwrap()
                .into(),
            BinOp::Lt => builder
                .build_float_compare(FloatPredicate::OLT, lhs, rhs, name)
                .unwrap()
                .into(),
            BinOp::Le => builder
                .build_float_compare(FloatPredicate::OLE, lhs, rhs, name)
                .unwrap()
                .into(),
            BinOp::Gt => builder
                .build_float_compare(FloatPredicate::OGT, lhs, rhs, name)
                .unwrap()
                .into(),
            BinOp::Ge => builder
                .build_float_compare(FloatPredicate::OGE, lhs, rhs, name)
                .unwrap()
                .into(),
            _ => panic!("build_float_binary: unsupported op {op:?}"),
        }
    }

    // ---- Unary operations ----

    pub fn build_unary<'ctx>(
        builder: &Builder<'ctx>,
        _context: &'ctx Context,
        op: UnaryOp,
        operand: BasicValueEnum<'ctx>,
        ty: &LltsType,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        match op {
            UnaryOp::Neg => {
                if TypeRegistry::is_integer(ty) {
                    builder
                        .build_int_neg(operand.into_int_value(), name)
                        .unwrap()
                        .into()
                } else {
                    builder
                        .build_float_neg(operand.into_float_value(), name)
                        .unwrap()
                        .into()
                }
            }
            UnaryOp::Not => {
                // Logical not: compare to zero / false.
                if TypeRegistry::is_integer(ty) || matches!(ty, LltsType::Bool) {
                    let v = operand.into_int_value();
                    let zero = v.get_type().const_int(0, false);
                    builder
                        .build_int_compare(IntPredicate::EQ, v, zero, name)
                        .unwrap()
                        .into()
                } else {
                    panic!("build_unary Not: unsupported type {ty:?}")
                }
            }
            UnaryOp::BitNot => {
                let v = operand.into_int_value();
                builder.build_not(v, name).unwrap().into()
            }
        }
    }

    // ---- Struct field access ----

    /// Emit a GEP into a struct to extract a field by index.
    pub fn build_struct_field_access<'ctx>(
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        struct_ptr: PointerValue<'ctx>,
        struct_type: &LltsType,
        field_index: u32,
        name: &str,
    ) -> PointerValue<'ctx> {
        let llvm_struct_ty = registry.llvm_type(struct_type);
        builder
            .build_struct_gep(llvm_struct_ty, struct_ptr, field_index, name)
            .unwrap()
    }

    /// Load a struct field value by index.
    pub fn build_load_struct_field<'ctx>(
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        struct_ptr: PointerValue<'ctx>,
        struct_type: &LltsType,
        field_index: u32,
        field_type: &LltsType,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        let field_ptr =
            Self::build_struct_field_access(builder, registry, struct_ptr, struct_type, field_index, name);
        let field_llvm_ty = registry.llvm_type(field_type);
        builder.build_load(field_llvm_ty, field_ptr, name).unwrap()
    }

    // ---- Array indexing ----

    /// Emit array element access with bounds checking.
    ///
    /// `array_ptr` points to an `{ ptr, len, cap }` struct. We extract the
    /// data pointer, check `index < len`, then GEP to the element.
    pub fn build_array_index<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        function: FunctionValue<'ctx>,
        array_val: BasicValueEnum<'ctx>,
        index: IntValue<'ctx>,
        elem_type: &LltsType,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        let arr = array_val.into_struct_value();

        // Extract data pointer and length.
        let data_ptr = builder
            .build_extract_value(arr, 0, "arr_data")
            .unwrap()
            .into_pointer_value();
        let len = builder
            .build_extract_value(arr, 1, "arr_len")
            .unwrap()
            .into_int_value();

        // Bounds check: index < len.
        let in_bounds = builder
            .build_int_compare(IntPredicate::ULT, index, len, "bounds_check")
            .unwrap();

        let access_bb = context.append_basic_block(function, "arr_access");
        let panic_bb = context.append_basic_block(function, "arr_oob");

        builder
            .build_conditional_branch(in_bounds, access_bb, panic_bb)
            .unwrap();

        // Out-of-bounds: trap.
        builder.position_at_end(panic_bb);
        // Call llvm.trap or just unreachable for now.
        builder.build_unreachable().unwrap();

        // In-bounds: GEP to element.
        builder.position_at_end(access_bb);
        let elem_llvm_ty = registry.llvm_type(elem_type);
        let elem_ptr = unsafe {
            builder
                .build_gep(elem_llvm_ty, data_ptr, &[index], "elem_ptr")
                .unwrap()
        };
        builder.build_load(elem_llvm_ty, elem_ptr, name).unwrap()
    }

    // ---- Type casts ----

    /// Build a type cast (as expression): integer widening/narrowing,
    /// int-to-float, float-to-int, float widening/narrowing.
    pub fn build_cast<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        value: BasicValueEnum<'ctx>,
        from: &LltsType,
        to: &LltsType,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        match (from, to) {
            // Int → wider int.
            (_, _) if TypeRegistry::is_integer(from) && TypeRegistry::is_integer(to) => {
                let v = value.into_int_value();
                let target_ty = match to {
                    LltsType::I8 | LltsType::U8 => context.i8_type(),
                    LltsType::I16 | LltsType::U16 => context.i16_type(),
                    LltsType::I32 | LltsType::U32 => context.i32_type(),
                    LltsType::I64 | LltsType::U64 => context.i64_type(),
                    _ => unreachable!(),
                };
                let from_width = v.get_type().get_bit_width();
                let to_width = target_ty.get_bit_width();
                if from_width == to_width {
                    value
                } else if from_width < to_width {
                    if TypeRegistry::is_signed(from) {
                        builder.build_int_s_extend(v, target_ty, name).unwrap().into()
                    } else {
                        builder.build_int_z_extend(v, target_ty, name).unwrap().into()
                    }
                } else {
                    builder.build_int_truncate(v, target_ty, name).unwrap().into()
                }
            }
            // Int → float.
            (_, _) if TypeRegistry::is_integer(from) && TypeRegistry::is_float(to) => {
                let v = value.into_int_value();
                let target_ty = if matches!(to, LltsType::F32) {
                    context.f32_type()
                } else {
                    context.f64_type()
                };
                if TypeRegistry::is_signed(from) {
                    builder
                        .build_signed_int_to_float(v, target_ty, name)
                        .unwrap()
                        .into()
                } else {
                    builder
                        .build_unsigned_int_to_float(v, target_ty, name)
                        .unwrap()
                        .into()
                }
            }
            // Float → int.
            (_, _) if TypeRegistry::is_float(from) && TypeRegistry::is_integer(to) => {
                let v = value.into_float_value();
                let target_ty = match to {
                    LltsType::I8 | LltsType::U8 => context.i8_type(),
                    LltsType::I16 | LltsType::U16 => context.i16_type(),
                    LltsType::I32 | LltsType::U32 => context.i32_type(),
                    LltsType::I64 | LltsType::U64 => context.i64_type(),
                    _ => unreachable!(),
                };
                if TypeRegistry::is_signed(to) {
                    builder
                        .build_float_to_signed_int(v, target_ty, name)
                        .unwrap()
                        .into()
                } else {
                    builder
                        .build_float_to_unsigned_int(v, target_ty, name)
                        .unwrap()
                        .into()
                }
            }
            // Float → float (f32 ↔ f64).
            (_, _) if TypeRegistry::is_float(from) && TypeRegistry::is_float(to) => {
                let v = value.into_float_value();
                if matches!(to, LltsType::F64) {
                    builder
                        .build_float_ext(v, context.f64_type(), name)
                        .unwrap()
                        .into()
                } else {
                    builder
                        .build_float_trunc(v, context.f32_type(), name)
                        .unwrap()
                        .into()
                }
            }
            _ => panic!("build_cast: unsupported cast from {from:?} to {to:?}"),
        }
    }

    // ---- Object/struct literal construction ----

    /// Build a struct literal: allocate on stack, store each field, return ptr.
    pub fn build_struct_literal<'ctx>(
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        struct_type: &LltsType,
        field_values: &[BasicValueEnum<'ctx>],
        name: &str,
    ) -> PointerValue<'ctx> {
        let llvm_ty = registry.llvm_type(struct_type);
        let alloca = builder.build_alloca(llvm_ty, name).unwrap();

        for (i, val) in field_values.iter().enumerate() {
            let field_ptr = builder
                .build_struct_gep(llvm_ty, alloca, i as u32, &format!("{name}_f{i}"))
                .unwrap();
            builder.build_store(field_ptr, *val).unwrap();
        }

        alloca
    }

    /// Build an array literal: malloc buffer, store elements, return { ptr, len, cap }.
    pub fn build_array_literal<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        module: &Module<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        elem_type: &LltsType,
        elements: &[BasicValueEnum<'ctx>],
        _name: &str,
    ) -> BasicValueEnum<'ctx> {
        let i64_ty = context.i64_type();
        let elem_llvm_ty = registry.llvm_type(elem_type);
        let count = elements.len() as u64;
        let elem_size = match elem_type {
            LltsType::I8 | LltsType::U8 | LltsType::Bool => 1u64,
            LltsType::I16 | LltsType::U16 => 2,
            LltsType::I32 | LltsType::U32 | LltsType::F32 => 4,
            LltsType::I64 | LltsType::U64 | LltsType::F64 => 8,
            _ => 8, // conservative default
        };
        let total_size = i64_ty.const_int(count * elem_size, false);

        // malloc(count * elem_size).
        let malloc_fn = module.get_function("malloc").unwrap();
        let data_ptr = builder
            .build_call(malloc_fn, &[total_size.into()], "arr_data")
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_pointer_value();

        // Store each element.
        for (i, val) in elements.iter().enumerate() {
            let idx = i64_ty.const_int(i as u64, false);
            let elem_ptr = unsafe {
                builder
                    .build_gep(elem_llvm_ty, data_ptr, &[idx], &format!("arr_el_{i}"))
                    .unwrap()
            };
            builder.build_store(elem_ptr, *val).unwrap();
        }

        // Build { ptr, len, cap } struct.
        let arr_ty = registry.array_type(elem_type);
        let arr_val = arr_ty.get_undef();
        let len_val = i64_ty.const_int(count, false);
        let cap_val = i64_ty.const_int(count, false);
        let arr_val = builder
            .build_insert_value(arr_val, data_ptr, 0, "arr_ptr")
            .unwrap()
            .into_struct_value();
        let arr_val = builder
            .build_insert_value(arr_val, len_val, 1, "arr_len")
            .unwrap()
            .into_struct_value();
        let arr_val = builder
            .build_insert_value(arr_val, cap_val, 2, "arr_cap")
            .unwrap()
            .into_struct_value();
        arr_val.into()
    }
}
