use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, PointerValue};
use inkwell::AddressSpace;

use crate::types::{LltsType, TypeRegistry};

/// Function call code generation.
///
/// Handles:
/// - Direct calls to known functions.
/// - Method calls (`obj.method()` → `Class_method(obj, args)`).
/// - Indirect calls through fat pointers (function values).
/// - Constructor calls (`new Foo()` → `Foo_new(args)`).
pub struct CallCodegen;

impl CallCodegen {
    /// Emit a direct function call to a known function.
    pub fn build_direct_call<'ctx>(
        builder: &Builder<'ctx>,
        function: FunctionValue<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let meta_args: Vec<BasicMetadataValueEnum<'ctx>> =
            args.iter().map(|a| (*a).into()).collect();
        let call = builder
            .build_call(function, &meta_args, name)
            .unwrap();
        call.try_as_basic_value().basic()
    }

    /// Emit a method call: `obj.method(args)` → `Class_method(obj, args)`.
    ///
    /// The method function must already be declared in the module with the
    /// mangled name `ClassName_methodName`. The receiver (`self`) is prepended
    /// to the argument list.
    pub fn build_method_call<'ctx>(
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        class_name: &str,
        method_name: &str,
        receiver: BasicValueEnum<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let mangled = format!("{class_name}_{method_name}");
        let function = module
            .get_function(&mangled)
            .unwrap_or_else(|| panic!("method not found: {mangled}"));

        let mut all_args: Vec<BasicMetadataValueEnum<'ctx>> = Vec::with_capacity(args.len() + 1);
        all_args.push(receiver.into());
        for a in args {
            all_args.push((*a).into());
        }

        let call = builder
            .build_call(function, &all_args, name)
            .unwrap();
        call.try_as_basic_value().basic()
    }

    /// Emit a constructor call: `new Foo(args)` → `Foo_new(args)`.
    ///
    /// The constructor must be a function `Foo_new` that returns the struct
    /// value (or pointer to it).
    pub fn build_constructor_call<'ctx>(
        builder: &Builder<'ctx>,
        module: &Module<'ctx>,
        class_name: &str,
        args: &[BasicValueEnum<'ctx>],
        name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let mangled = format!("{class_name}_new");
        let function = module
            .get_function(&mangled)
            .unwrap_or_else(|| panic!("constructor not found: {mangled}"));

        let meta_args: Vec<BasicMetadataValueEnum<'ctx>> =
            args.iter().map(|a| (*a).into()).collect();
        let call = builder
            .build_call(function, &meta_args, name)
            .unwrap();
        call.try_as_basic_value().basic()
    }

    /// Emit an indirect call through a fat pointer (function value).
    ///
    /// A function value is `{ fn_ptr, env_ptr }`. We extract both, then call
    /// `fn_ptr(env_ptr, args...)`. The `env_ptr` is passed as a hidden first
    /// argument (it's null for plain functions, and points to captured
    /// variables for closures).
    pub fn build_fat_ptr_call<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        fat_ptr: BasicValueEnum<'ctx>,
        args: &[BasicValueEnum<'ctx>],
        param_types: &[LltsType],
        ret_type: &LltsType,
        name: &str,
    ) -> Option<BasicValueEnum<'ctx>> {
        let fat = fat_ptr.into_struct_value();

        // Extract fn_ptr and env_ptr.
        let fn_ptr = builder
            .build_extract_value(fat, 0, "fn_ptr")
            .unwrap()
            .into_pointer_value();
        let env_ptr = builder
            .build_extract_value(fat, 1, "env_ptr")
            .unwrap();

        // Build the full argument list: [env_ptr, args...].
        let _ptr_ty = context.ptr_type(AddressSpace::default());
        let mut all_param_types: Vec<LltsType> = Vec::with_capacity(param_types.len() + 1);
        all_param_types.push(LltsType::Ptr); // env_ptr
        all_param_types.extend_from_slice(param_types);

        let fn_type = registry.fn_type(&all_param_types, ret_type);

        let mut all_args: Vec<BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(args.len() + 1);
        all_args.push(env_ptr.into());
        for a in args {
            all_args.push((*a).into());
        }

        let call = builder
            .build_indirect_call(fn_type, fn_ptr, &all_args, name)
            .unwrap();
        call.try_as_basic_value().basic()
    }

    /// Create a fat pointer for a known function (non-closure: env_ptr = null).
    pub fn build_fn_fat_ptr<'ctx>(
        builder: &Builder<'ctx>,
        context: &'ctx Context,
        registry: &mut TypeRegistry<'ctx>,
        function: FunctionValue<'ctx>,
        params: &[LltsType],
        ret: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        let fat_ty = registry.fat_fn_type(params, ret);
        let fn_ptr = function.as_global_value().as_pointer_value();
        let null_env = context
            .ptr_type(AddressSpace::default())
            .const_null();

        let val = fat_ty.get_undef();
        let val = builder
            .build_insert_value(val, fn_ptr, 0, "fat_fn")
            .unwrap()
            .into_struct_value();
        let val = builder
            .build_insert_value(val, null_env, 1, "fat_env")
            .unwrap()
            .into_struct_value();
        val.into()
    }

    /// Create a fat pointer for a closure (env_ptr points to captured vars).
    pub fn build_closure_fat_ptr<'ctx>(
        builder: &Builder<'ctx>,
        registry: &mut TypeRegistry<'ctx>,
        function: FunctionValue<'ctx>,
        env_ptr: PointerValue<'ctx>,
        params: &[LltsType],
        ret: &LltsType,
    ) -> BasicValueEnum<'ctx> {
        let fat_ty = registry.fat_fn_type(params, ret);
        let fn_ptr = function.as_global_value().as_pointer_value();

        let val = fat_ty.get_undef();
        let val = builder
            .build_insert_value(val, fn_ptr, 0, "closure_fn")
            .unwrap()
            .into_struct_value();
        let val = builder
            .build_insert_value(val, env_ptr, 1, "closure_env")
            .unwrap()
            .into_struct_value();
        val.into()
    }
}
