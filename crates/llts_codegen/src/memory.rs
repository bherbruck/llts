use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::AddressSpace;

use crate::types::{LltsType, TypeRegistry};

/// Handles memory operations in generated code: stack allocation, heap
/// allocation via libc malloc/free, and reference counting retain/release.
pub struct MemoryManager<'ctx> {
    context: &'ctx Context,
    /// Cached declaration of libc `malloc`.
    malloc_fn: Option<FunctionValue<'ctx>>,
    /// Cached declaration of libc `free`.
    free_fn: Option<FunctionValue<'ctx>>,
}

impl<'ctx> MemoryManager<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        Self {
            context,
            malloc_fn: None,
            free_fn: None,
        }
    }

    // ---- Stack allocation ----

    /// Emit an `alloca` for a local variable of the given type.
    pub fn build_stack_alloc(
        &self,
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        ty: &LltsType,
        name: &str,
    ) -> PointerValue<'ctx> {
        let llvm_ty = registry.llvm_type(ty);
        builder.build_alloca(llvm_ty, name).unwrap()
    }

    /// Emit a `store` of `value` into `ptr`.
    pub fn build_store(
        &self,
        builder: &Builder<'ctx>,
        ptr: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
    ) {
        builder.build_store(ptr, value).unwrap();
    }

    /// Emit a `load` from `ptr` with the given type.
    pub fn build_load(
        &self,
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        ty: &LltsType,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        let llvm_ty = registry.llvm_type(ty);
        builder.build_load(llvm_ty, ptr, name).unwrap()
    }

    // ---- Heap allocation ----

    /// Ensure `malloc` is declared in the module and return it.
    pub fn get_or_declare_malloc(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(f) = self.malloc_fn {
            return f;
        }
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let fn_type = ptr_ty.fn_type(&[i64_ty.into()], false);
        let f = module.add_function("malloc", fn_type, None);
        self.malloc_fn = Some(f);
        f
    }

    /// Ensure `free` is declared in the module and return it.
    pub fn get_or_declare_free(&mut self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
        if let Some(f) = self.free_fn {
            return f;
        }
        let ptr_ty = self.context.ptr_type(AddressSpace::default());
        let void_ty = self.context.void_type();
        let fn_type = void_ty.fn_type(&[ptr_ty.into()], false);
        let f = module.add_function("free", fn_type, None);
        self.free_fn = Some(f);
        f
    }

    /// Emit a call to `malloc(size)` and return the resulting pointer.
    pub fn build_heap_alloc(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        size: IntValue<'ctx>,
        name: &str,
    ) -> PointerValue<'ctx> {
        let malloc = self.get_or_declare_malloc(module);
        let call = builder
            .build_call(malloc, &[size.into()], name)
            .unwrap();
        call.try_as_basic_value()
            .unwrap_basic()
            .into_pointer_value()
    }

    /// Emit a call to `free(ptr)`.
    pub fn build_heap_free(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        ptr: PointerValue<'ctx>,
    ) {
        let free = self.get_or_declare_free(module);
        builder.build_call(free, &[ptr.into()], "").unwrap();
    }

    // ---- Reference counting ----

    /// Build a retain (increment reference count) call.
    ///
    /// The RC header is stored immediately before the allocation pointer:
    /// `[rc: i64][data...]` where `ptr` points to `data`. We GEP backwards
    /// to find the count and atomically increment it.
    ///
    /// For now this is a simple non-atomic increment suitable for
    /// single-threaded execution.
    pub fn build_retain(
        &self,
        builder: &Builder<'ctx>,
        ptr: PointerValue<'ctx>,
    ) {
        let i64_ty = self.context.i64_type();
        // GEP to the refcount slot: ptr - 8 bytes.
        let neg_one = i64_ty.const_int(u64::MAX, false); // -1 as unsigned
        let rc_ptr = unsafe {
            builder
                .build_gep(i64_ty, ptr, &[neg_one], "rc_ptr")
                .unwrap()
        };
        let rc = builder
            .build_load(i64_ty, rc_ptr, "rc")
            .unwrap()
            .into_int_value();
        let one = i64_ty.const_int(1, false);
        let new_rc = builder.build_int_add(rc, one, "rc_inc").unwrap();
        builder.build_store(rc_ptr, new_rc).unwrap();
    }

    /// Build a release (decrement reference count) call. If the count reaches
    /// zero, call the destructor / free.
    ///
    /// Returns the basic block that continues after the check so the caller
    /// can position there.
    pub fn build_release(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        function: FunctionValue<'ctx>,
        ptr: PointerValue<'ctx>,
    ) {
        let i64_ty = self.context.i64_type();
        let neg_one = i64_ty.const_int(u64::MAX, false);
        let rc_ptr = unsafe {
            builder
                .build_gep(i64_ty, ptr, &[neg_one], "rc_ptr")
                .unwrap()
        };
        let rc = builder
            .build_load(i64_ty, rc_ptr, "rc")
            .unwrap()
            .into_int_value();
        let one = i64_ty.const_int(1, false);
        let new_rc = builder.build_int_sub(rc, one, "rc_dec").unwrap();
        builder.build_store(rc_ptr, new_rc).unwrap();

        // Branch: if new_rc == 0 then free.
        let zero = i64_ty.const_int(0, false);
        let is_zero = builder
            .build_int_compare(inkwell::IntPredicate::EQ, new_rc, zero, "rc_is_zero")
            .unwrap();

        let free_bb = self.context.append_basic_block(function, "rc_free");
        let cont_bb = self.context.append_basic_block(function, "rc_cont");

        builder
            .build_conditional_branch(is_zero, free_bb, cont_bb)
            .unwrap();

        // Free block.
        builder.position_at_end(free_bb);
        // Free the original allocation (ptr - 8 for RC header).
        let alloc_ptr = unsafe {
            builder
                .build_gep(self.context.i8_type(), ptr, &[neg_one], "alloc_ptr")
                .unwrap()
        };
        self.build_heap_free(builder, module, alloc_ptr);
        builder.build_unconditional_branch(cont_bb).unwrap();

        // Continue.
        builder.position_at_end(cont_bb);
    }

    // ---- String allocation ----

    /// Allocate a string on the heap: malloc(len), memcpy data, return { ptr, len }.
    pub fn build_string_alloc(
        &mut self,
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        registry: &TypeRegistry<'ctx>,
        data_ptr: PointerValue<'ctx>,
        len: IntValue<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        let heap_ptr = self.build_heap_alloc(builder, module, len, "str_alloc");

        // Declare or get memcpy.
        let memcpy = self.get_or_declare_memcpy(module);
        // false = not volatile
        builder
            .build_call(
                memcpy,
                &[heap_ptr.into(), data_ptr.into(), len.into()],
                "",
            )
            .unwrap();

        let str_ty = registry.string_type();
        let str_val = str_ty.get_undef();
        let str_val = builder
            .build_insert_value(str_val, heap_ptr, 0, "str_ptr")
            .unwrap()
            .into_struct_value();
        let str_val = builder
            .build_insert_value(str_val, len, 1, "str_len")
            .unwrap()
            .into_struct_value();
        str_val.into()
    }

    fn get_or_declare_memcpy(&self, module: &Module<'ctx>) -> FunctionValue<'ctx> {
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
}
