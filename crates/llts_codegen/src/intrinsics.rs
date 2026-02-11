use std::collections::HashMap;

use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue};
use inkwell::AddressSpace;

use crate::types::TypeRegistry;

/// Manages compiler intrinsic functions: print/console.log, math builtins,
/// memory primitives, and string operations.
pub struct Intrinsics<'ctx> {
    context: &'ctx Context,
    /// Cache of declared intrinsic functions by name.
    cache: HashMap<String, FunctionValue<'ctx>>,
}

impl<'ctx> Intrinsics<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        Self {
            context,
            cache: HashMap::new(),
        }
    }

    /// Declare all standard intrinsics in the module. Called once during
    /// codegen initialization (pass 2).
    pub fn declare_all(&mut self, module: &Module<'ctx>) {
        self.declare_write(module);
        self.declare_snprintf(module);
        self.declare_math_intrinsics(module);
        self.declare_puts(module);
        self.declare_exit(module);
        self.declare_setjmp(module);
        self.declare_longjmp(module);
        self.declare_memcpy(module);
    }

    // ---- I/O: print / console.log ----

    /// `write(fd, buf, count) -> ssize_t` — POSIX write syscall.
    fn declare_write(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(&f) = self.cache.get("write") {
            return f;
        }
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let fn_type = i64_ty.fn_type(&[i32_ty.into(), ptr_ty.into(), i64_ty.into()], false);
        let f = module.add_function("write", fn_type, None);
        self.cache.insert("write".to_string(), f);
        f
    }

    /// `puts(s) -> i32` — libc puts (prints string + newline).
    fn declare_puts(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(&f) = self.cache.get("puts") {
            return f;
        }
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let fn_type = i32_ty.fn_type(&[ptr_ty.into()], false);
        let f = module.add_function("puts", fn_type, None);
        self.cache.insert("puts".to_string(), f);
        f
    }

    /// `snprintf(buf, size, fmt, ...) -> i32` — for number formatting.
    fn declare_snprintf(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(&f) = self.cache.get("snprintf") {
            return f;
        }
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        // snprintf is variadic.
        let fn_type = i32_ty.fn_type(&[ptr_ty.into(), i64_ty.into(), ptr_ty.into()], true);
        let f = module.add_function("snprintf", fn_type, None);
        self.cache.insert("snprintf".to_string(), f);
        f
    }

    /// Emit a `print(str)` call — writes the string fat pointer to stdout.
    ///
    /// `str_val` must be a struct value `{ ptr, len }`.
    pub fn build_print_string(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        str_val: BasicValueEnum<'ctx>,
    ) {
        let write_fn = self.declare_write(module);
        let i32_ty = self.context.i32_type();

        // Extract ptr and len from the string struct.
        let str_agg = str_val.into_struct_value();
        let ptr = builder
            .build_extract_value(str_agg, 0, "str_ptr")
            .unwrap()
            .into_pointer_value();
        let len = builder
            .build_extract_value(str_agg, 1, "str_len")
            .unwrap()
            .into_int_value();

        // write(1 /* stdout */, ptr, len)
        let stdout = i32_ty.const_int(1, false);
        builder
            .build_call(write_fn, &[stdout.into(), ptr.into(), len.into()], "")
            .unwrap();
    }

    /// Emit a `print(i32_value)` — format the integer and write to stdout.
    pub fn build_print_i32(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        value: IntValue<'ctx>,
    ) {
        let snprintf = self.declare_snprintf(module);
        let write_fn = self.declare_write(module);

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();

        // Stack buffer for formatted number.
        let buf = builder
            .build_alloca(self.context.i8_type().array_type(32), "fmt_buf")
            .unwrap();
        let buf_ptr = builder
            .build_pointer_cast(buf, self.context.ptr_type(AddressSpace::default()), "buf_ptr")
            .unwrap();
        let buf_size = i64_ty.const_int(32, false);

        // Format string "%d\n".
        let fmt = builder
            .build_global_string_ptr("%d\n", "fmt_i32")
            .unwrap()
            .as_pointer_value();

        // snprintf(buf, 32, "%d\n", value)
        let len = builder
            .build_call(
                snprintf,
                &[buf_ptr.into(), buf_size.into(), fmt.into(), value.into()],
                "fmt_len",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();

        // write(1, buf, len)
        let stdout = i32_ty.const_int(1, false);
        let len_i64 = builder
            .build_int_z_extend(len, i64_ty, "len_i64")
            .unwrap();
        builder
            .build_call(
                write_fn,
                &[stdout.into(), buf_ptr.into(), len_i64.into()],
                "",
            )
            .unwrap();
    }

    /// Emit a `print(u32_value)` — format the unsigned integer and write to stdout.
    pub fn build_print_u32(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        value: IntValue<'ctx>,
    ) {
        let snprintf = self.declare_snprintf(module);
        let write_fn = self.declare_write(module);

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();

        // Stack buffer for formatted number.
        let buf = builder
            .build_alloca(self.context.i8_type().array_type(32), "fmt_buf")
            .unwrap();
        let buf_ptr = builder
            .build_pointer_cast(buf, self.context.ptr_type(AddressSpace::default()), "buf_ptr")
            .unwrap();
        let buf_size = i64_ty.const_int(32, false);

        // Format string "%u\n".
        let fmt = builder
            .build_global_string_ptr("%u\n", "fmt_u32")
            .unwrap()
            .as_pointer_value();

        // snprintf(buf, 32, "%u\n", value)
        let len = builder
            .build_call(
                snprintf,
                &[buf_ptr.into(), buf_size.into(), fmt.into(), value.into()],
                "fmt_len",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();

        // write(1, buf, len)
        let stdout = i32_ty.const_int(1, false);
        let len_i64 = builder
            .build_int_z_extend(len, i64_ty, "len_i64")
            .unwrap();
        builder
            .build_call(
                write_fn,
                &[stdout.into(), buf_ptr.into(), len_i64.into()],
                "",
            )
            .unwrap();
    }

    /// Emit a `print(f64_value)` — format the float and write to stdout.
    pub fn build_print_f64(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) {
        let snprintf = self.declare_snprintf(module);
        let write_fn = self.declare_write(module);

        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();

        let buf = builder
            .build_alloca(self.context.i8_type().array_type(64), "fmt_buf")
            .unwrap();
        let buf_ptr = builder
            .build_pointer_cast(buf, self.context.ptr_type(AddressSpace::default()), "buf_ptr")
            .unwrap();
        let buf_size = i64_ty.const_int(64, false);

        let fmt = builder
            .build_global_string_ptr("%.15g\n", "fmt_f64")
            .unwrap()
            .as_pointer_value();

        let len = builder
            .build_call(
                snprintf,
                &[buf_ptr.into(), buf_size.into(), fmt.into(), value.into()],
                "fmt_len",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();

        let stdout = i32_ty.const_int(1, false);
        let len_i64 = builder
            .build_int_z_extend(len, i64_ty, "len_i64")
            .unwrap();
        builder
            .build_call(
                write_fn,
                &[stdout.into(), buf_ptr.into(), len_i64.into()],
                "",
            )
            .unwrap();
    }

    /// Print an integer without a trailing newline (for struct field printing).
    pub fn build_print_i32_inline(
        &mut self, builder: &Builder<'ctx>, module: &Module<'ctx>, value: IntValue<'ctx>,
    ) {
        let snprintf = self.declare_snprintf(module);
        let write_fn = self.declare_write(module);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let buf = builder.build_alloca(self.context.i8_type().array_type(32), "fmt_buf").unwrap();
        let buf_ptr = builder.build_pointer_cast(buf, self.context.ptr_type(AddressSpace::default()), "buf_ptr").unwrap();
        let buf_size = i64_ty.const_int(32, false);
        let fmt = builder.build_global_string_ptr("%d", "fmt_i32_nl").unwrap().as_pointer_value();
        let len = builder.build_call(snprintf, &[buf_ptr.into(), buf_size.into(), fmt.into(), value.into()], "fmt_len")
            .unwrap().try_as_basic_value().unwrap_basic().into_int_value();
        let stdout = i32_ty.const_int(1, false);
        let len_i64 = builder.build_int_z_extend(len, i64_ty, "len_i64").unwrap();
        builder.build_call(write_fn, &[stdout.into(), buf_ptr.into(), len_i64.into()], "").unwrap();
    }

    /// Print an unsigned integer without a trailing newline.
    pub fn build_print_u32_inline(
        &mut self, builder: &Builder<'ctx>, module: &Module<'ctx>, value: IntValue<'ctx>,
    ) {
        let snprintf = self.declare_snprintf(module);
        let write_fn = self.declare_write(module);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let buf = builder.build_alloca(self.context.i8_type().array_type(32), "fmt_buf").unwrap();
        let buf_ptr = builder.build_pointer_cast(buf, self.context.ptr_type(AddressSpace::default()), "buf_ptr").unwrap();
        let buf_size = i64_ty.const_int(32, false);
        let fmt = builder.build_global_string_ptr("%u", "fmt_u32_nl").unwrap().as_pointer_value();
        let len = builder.build_call(snprintf, &[buf_ptr.into(), buf_size.into(), fmt.into(), value.into()], "fmt_len")
            .unwrap().try_as_basic_value().unwrap_basic().into_int_value();
        let stdout = i32_ty.const_int(1, false);
        let len_i64 = builder.build_int_z_extend(len, i64_ty, "len_i64").unwrap();
        builder.build_call(write_fn, &[stdout.into(), buf_ptr.into(), len_i64.into()], "").unwrap();
    }

    /// Print a float without a trailing newline.
    pub fn build_print_f64_inline(
        &mut self, builder: &Builder<'ctx>, module: &Module<'ctx>, value: BasicValueEnum<'ctx>,
    ) {
        let snprintf = self.declare_snprintf(module);
        let write_fn = self.declare_write(module);
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let buf = builder.build_alloca(self.context.i8_type().array_type(64), "fmt_buf").unwrap();
        let buf_ptr = builder.build_pointer_cast(buf, self.context.ptr_type(AddressSpace::default()), "buf_ptr").unwrap();
        let buf_size = i64_ty.const_int(64, false);
        let fmt = builder.build_global_string_ptr("%.15g", "fmt_f64_nl").unwrap().as_pointer_value();
        let len = builder.build_call(snprintf, &[buf_ptr.into(), buf_size.into(), fmt.into(), value.into()], "fmt_len")
            .unwrap().try_as_basic_value().unwrap_basic().into_int_value();
        let stdout = i32_ty.const_int(1, false);
        let len_i64 = builder.build_int_z_extend(len, i64_ty, "len_i64").unwrap();
        builder.build_call(write_fn, &[stdout.into(), buf_ptr.into(), len_i64.into()], "").unwrap();
    }

    // ---- Math intrinsics ----

    fn declare_math_intrinsics(&mut self, module: &Module<'ctx>) {
        let f64_ty = self.context.f64_type();

        // Single-arg math functions: sqrt, abs, floor, ceil, sin, cos, log, exp.
        let unary_names = [
            "llvm.sqrt.f64",
            "llvm.fabs.f64",
            "llvm.floor.f64",
            "llvm.ceil.f64",
            "sin",
            "cos",
            "log",
            "exp",
        ];
        for name in unary_names {
            let fn_type = f64_ty.fn_type(&[f64_ty.into()], false);
            let f = module.add_function(name, fn_type, None);
            self.cache.insert(name.to_string(), f);
        }

        // Additional unary math: round, trunc
        let extra_unary = ["llvm.round.f64", "llvm.trunc.f64"];
        for name in extra_unary {
            let fn_type = f64_ty.fn_type(&[f64_ty.into()], false);
            let f = module.add_function(name, fn_type, None);
            self.cache.insert(name.to_string(), f);
        }

        // pow(base, exp) -> f64
        let pow_type = f64_ty.fn_type(&[f64_ty.into(), f64_ty.into()], false);
        let f = module.add_function("llvm.pow.f64", pow_type, None);
        self.cache.insert("llvm.pow.f64".to_string(), f);

        // rand() -> i32 (libc)
        let i32_ty = self.context.i32_type();
        let rand_type = i32_ty.fn_type(&[], false);
        let f = module.add_function("rand", rand_type, None);
        self.cache.insert("rand".to_string(), f);

        // srand(seed: u32) -> void (libc)
        let void_ty = self.context.void_type();
        let srand_type = void_ty.fn_type(&[i32_ty.into()], false);
        let f = module.add_function("srand", srand_type, None);
        self.cache.insert("srand".to_string(), f);

        // fmin(a, b) -> f64 and fmax(a, b) -> f64 (libc)
        let binary_f64_type = f64_ty.fn_type(&[f64_ty.into(), f64_ty.into()], false);
        let f = module.add_function("fmin", binary_f64_type, None);
        self.cache.insert("fmin".to_string(), f);
        let f = module.add_function("fmax", binary_f64_type, None);
        self.cache.insert("fmax".to_string(), f);
    }

    /// Get a cached intrinsic by name, or `None` if it hasn't been declared.
    pub fn get(&self, name: &str) -> Option<FunctionValue<'ctx>> {
        self.cache.get(name).copied()
    }

    /// Build a call to a unary math intrinsic (sqrt, fabs, floor, ceil, sin, cos, log, exp).
    pub fn build_math_unary(
        &self,
        builder: &Builder<'ctx>,
        name: &str,
        arg: BasicValueEnum<'ctx>,
        result_name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let f = self.cache.get(name)?;
        let call = builder
            .build_call(*f, &[arg.into()], result_name)
            .unwrap();
        call.try_as_basic_value().basic()
    }

    /// Build a call to `pow(base, exp)`.
    pub fn build_math_pow(
        &self,
        builder: &Builder<'ctx>,
        base: BasicValueEnum<'ctx>,
        exp: BasicValueEnum<'ctx>,
        result_name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let f = self.cache.get("llvm.pow.f64")?;
        let call = builder
            .build_call(*f, &[base.into(), exp.into()], result_name)
            .unwrap();
        call.try_as_basic_value().basic()
    }

    // ---- String operations ----

    /// Build string concatenation: allocate new buffer, memcpy both halves.
    pub fn build_string_concat(
        &self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        // Extract ptr/len from both.
        let lhs_s = lhs.into_struct_value();
        let rhs_s = rhs.into_struct_value();
        let lhs_ptr = builder
            .build_extract_value(lhs_s, 0, "lhs_ptr")
            .unwrap()
            .into_pointer_value();
        let lhs_len = builder
            .build_extract_value(lhs_s, 1, "lhs_len")
            .unwrap()
            .into_int_value();
        let rhs_ptr = builder
            .build_extract_value(rhs_s, 0, "rhs_ptr")
            .unwrap()
            .into_pointer_value();
        let rhs_len = builder
            .build_extract_value(rhs_s, 1, "rhs_len")
            .unwrap()
            .into_int_value();

        // Total length.
        let total_len = builder
            .build_int_add(lhs_len, rhs_len, "total_len")
            .unwrap();

        // malloc(total_len).
        let malloc_fn = module.get_function("malloc").unwrap();
        let new_buf = builder
            .build_call(malloc_fn, &[total_len.into()], "concat_buf")
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_pointer_value();

        // memcpy first half.
        let memcpy_fn = module.get_function("memcpy").unwrap();
        builder
            .build_call(
                memcpy_fn,
                &[new_buf.into(), lhs_ptr.into(), lhs_len.into()],
                "",
            )
            .unwrap();

        // GEP to second half.
        let dest2 = unsafe {
            builder
                .build_gep(self.context.i8_type(), new_buf, &[lhs_len], "dest2")
                .unwrap()
        };
        builder
            .build_call(
                memcpy_fn,
                &[dest2.into(), rhs_ptr.into(), rhs_len.into()],
                "",
            )
            .unwrap();

        // Build result struct { ptr, len }.
        let str_ty = registry.string_type();
        let str_val = str_ty.get_undef();
        let str_val = builder
            .build_insert_value(str_val, new_buf, 0, "concat_ptr")
            .unwrap()
            .into_struct_value();
        let str_val = builder
            .build_insert_value(str_val, total_len, 1, "concat_len")
            .unwrap()
            .into_struct_value();
        str_val.into()
    }

    /// Build string length: extract the `len` field.
    pub fn build_string_length(
        &self,
        builder: &Builder<'ctx>,
        str_val: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        let s = str_val.into_struct_value();
        builder
            .build_extract_value(s, 1, "str_len")
            .unwrap()
            .into_int_value()
    }

    /// Build string equality comparison: compare lengths, then memcmp.
    pub fn build_string_eq(
        &self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        lhs: BasicValueEnum<'ctx>,
        rhs: BasicValueEnum<'ctx>,
    ) -> IntValue<'ctx> {
        let lhs_s = lhs.into_struct_value();
        let rhs_s = rhs.into_struct_value();
        let lhs_len = builder
            .build_extract_value(lhs_s, 1, "lhs_len")
            .unwrap()
            .into_int_value();
        let rhs_len = builder
            .build_extract_value(rhs_s, 1, "rhs_len")
            .unwrap()
            .into_int_value();

        // Compare lengths first.
        let len_eq = builder
            .build_int_compare(
                inkwell::IntPredicate::EQ,
                lhs_len,
                rhs_len,
                "len_eq",
            )
            .unwrap();

        let cmp_bb = self.context.append_basic_block(function, "str_cmp");
        let done_bb = self.context.append_basic_block(function, "str_eq_done");

        builder
            .build_conditional_branch(len_eq, cmp_bb, done_bb)
            .unwrap();

        // Compare contents with memcmp.
        builder.position_at_end(cmp_bb);
        let lhs_ptr = builder
            .build_extract_value(lhs_s, 0, "lhs_ptr")
            .unwrap();
        let rhs_ptr = builder
            .build_extract_value(rhs_s, 0, "rhs_ptr")
            .unwrap();

        let memcmp = self.get_or_declare_memcmp(module);
        let cmp_result = builder
            .build_call(
                memcmp,
                &[lhs_ptr.into(), rhs_ptr.into(), lhs_len.into()],
                "memcmp_res",
            )
            .unwrap()
            .try_as_basic_value()
            .unwrap_basic()
            .into_int_value();
        let zero = self.context.i32_type().const_int(0, false);
        let bytes_eq = builder
            .build_int_compare(inkwell::IntPredicate::EQ, cmp_result, zero, "bytes_eq")
            .unwrap();
        builder.build_unconditional_branch(done_bb).unwrap();

        // Phi node: false from length-mismatch path, bytes_eq from cmp path.
        builder.position_at_end(done_bb);
        let phi = builder
            .build_phi(self.context.bool_type(), "str_eq")
            .unwrap();
        let false_val = self.context.bool_type().const_int(0, false);
        let entry_bb = cmp_bb
            .get_previous_basic_block()
            .unwrap_or(function.get_first_basic_block().unwrap());
        phi.add_incoming(&[(&false_val, entry_bb), (&bytes_eq, cmp_bb)]);
        phi.as_basic_value().into_int_value()
    }

    fn get_or_declare_memcmp(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(f) = module.get_function("memcmp") {
            return f;
        }
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let fn_type = i32_ty.fn_type(
            &[ptr_ty.into(), ptr_ty.into(), i64_ty.into()],
            false,
        );
        module.add_function("memcmp", fn_type, None)
    }

    /// `memcpy(dest, src, n) -> ptr` — libc memcpy, used by string concatenation.
    fn declare_memcpy(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(f) = module.get_function("memcpy") {
            return f;
        }
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let fn_type = ptr_ty.fn_type(
            &[ptr_ty.into(), ptr_ty.into(), i64_ty.into()],
            false,
        );
        module.add_function("memcpy", fn_type, None)
    }

    // ---- Error handling intrinsics (setjmp/longjmp/exit) ----

    /// `exit(status: i32) -> void` — libc exit.
    fn declare_exit(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(&f) = self.cache.get("exit") {
            return f;
        }
        let i32_ty = self.context.i32_type();
        let void_ty = self.context.void_type();
        let fn_type = void_ty.fn_type(&[i32_ty.into()], false);
        let f = module.add_function("exit", fn_type, None);
        self.cache.insert("exit".to_string(), f);
        f
    }

    /// `setjmp(buf: *i8) -> i32` — POSIX setjmp.
    fn declare_setjmp(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(&f) = self.cache.get("setjmp") {
            return f;
        }
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let fn_type = i32_ty.fn_type(&[ptr_ty.into()], false);
        let f = module.add_function("setjmp", fn_type, None);
        self.cache.insert("setjmp".to_string(), f);
        f
    }

    /// `longjmp(buf: *i8, val: i32) -> void` — POSIX longjmp.
    fn declare_longjmp(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(&f) = self.cache.get("longjmp") {
            return f;
        }
        let i32_ty = self.context.i32_type();
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let void_ty = self.context.void_type();
        let fn_type = void_ty.fn_type(&[ptr_ty.into(), i32_ty.into()], false);
        let f = module.add_function("longjmp", fn_type, None);
        self.cache.insert("longjmp".to_string(), f);
        f
    }

    /// Emit a write to stderr (fd=2). `str_val` must be a string fat pointer `{ ptr, len }`.
    pub fn build_write_stderr(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        str_val: BasicValueEnum<'ctx>,
    ) {
        let write_fn = self.declare_write(module);
        let i32_ty = self.context.i32_type();

        let str_agg = str_val.into_struct_value();
        let ptr = builder
            .build_extract_value(str_agg, 0, "err_ptr")
            .unwrap()
            .into_pointer_value();
        let len = builder
            .build_extract_value(str_agg, 1, "err_len")
            .unwrap()
            .into_int_value();

        // write(2 /* stderr */, ptr, len)
        let stderr_fd = i32_ty.const_int(2, false);
        builder
            .build_call(write_fn, &[stderr_fd.into(), ptr.into(), len.into()], "")
            .unwrap();
    }
}
