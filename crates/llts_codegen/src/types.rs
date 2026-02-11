use std::collections::HashMap;

use inkwell::context::Context;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FunctionType, StructType};
use inkwell::AddressSpace;

/// Compiler-level type representation.
///
/// This mirrors the type system described in the LLTS design docs. When the
/// `llts_analysis` crate stabilizes its `LltsType` enum, this can be replaced
/// or bridged. For now codegen defines its own so it can compile independently.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LltsType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
    Void,
    Never,
    /// Fat pointer: { ptr, len } pointing to UTF-8 data.
    String,
    /// Named struct with ordered fields.
    Struct {
        name: String,
        fields: Vec<(String, LltsType)>,
    },
    /// Growable array: { ptr, len, cap }.
    Array(Box<LltsType>),
    /// Optional value: { i1, T }.
    Option(Box<LltsType>),
    /// Result type: { i32_tag, union(T, E) }.
    Result {
        ok: Box<LltsType>,
        err: Box<LltsType>,
    },
    /// Function value (fat pointer): { fn_ptr, env_ptr }.
    Function {
        params: Vec<LltsType>,
        ret: Box<LltsType>,
    },
    /// Tagged union: { i32_tag, max_variant_bytes }.
    Union {
        name: String,
        variants: Vec<(String, LltsType)>,
    },
    /// Raw pointer (used internally for env_ptr, data pointers, etc.).
    Ptr,
}

/// Registry that maps [`LltsType`] values to LLVM types for a given context.
///
/// The registry caches struct types so that recursive references and repeated
/// lookups are efficient. It is created once per module and threaded through
/// all codegen passes.
pub struct TypeRegistry<'ctx> {
    context: &'ctx Context,
    /// Cache of named struct LLVM types.
    struct_cache: HashMap<String, StructType<'ctx>>,
}

impl<'ctx> TypeRegistry<'ctx> {
    pub fn new(context: &'ctx Context) -> Self {
        Self {
            context,
            struct_cache: HashMap::new(),
        }
    }

    /// Convert a compiler type to its LLVM representation.
    pub fn llvm_type(&mut self, ty: &LltsType) -> BasicTypeEnum<'ctx> {
        match ty {
            LltsType::I8 | LltsType::U8 => self.context.i8_type().into(),
            LltsType::I16 | LltsType::U16 => self.context.i16_type().into(),
            LltsType::I32 | LltsType::U32 => self.context.i32_type().into(),
            LltsType::I64 | LltsType::U64 => self.context.i64_type().into(),
            LltsType::F32 => self.context.f32_type().into(),
            LltsType::F64 => self.context.f64_type().into(),
            LltsType::Bool => self.context.bool_type().into(),
            LltsType::Void => {
                // Void has no value representation; use i8 as a placeholder
                // when a BasicTypeEnum is required (e.g. alloca for unit-typed
                // variables). Real void returns use build_return(None).
                self.context.i8_type().into()
            }
            LltsType::Never => {
                // Never-returning functions are marked with unreachable; use i8
                // as a placeholder type.
                self.context.i8_type().into()
            }
            LltsType::String => self.string_type().into(),
            LltsType::Struct { name, fields } => self.struct_type(name, fields).into(),
            LltsType::Array(elem) => self.array_type(elem).into(),
            LltsType::Option(inner) => self.option_type(inner).into(),
            LltsType::Result { ok, err } => self.result_type(ok, err).into(),
            LltsType::Function { params, ret } => self.fat_fn_type(params, ret).into(),
            LltsType::Union { name, variants } => self.union_type(name, variants).into(),
            LltsType::Ptr => self.context.ptr_type(AddressSpace::default()).into(),
        }
    }

    /// Build a function type from compiler-level param/return types.
    ///
    /// Unlike [`Self::llvm_type`] this returns an `inkwell::FunctionType`
    /// rather than a `BasicTypeEnum`, because LLVM function types are not
    /// basic types. Void returns are handled by emitting an LLVM void
    /// function type.
    pub fn fn_type(
        &mut self,
        params: &[LltsType],
        ret: &LltsType,
    ) -> FunctionType<'ctx> {
        let param_types: Vec<BasicMetadataTypeEnum<'ctx>> = params
            .iter()
            .map(|p| self.llvm_type(p).into())
            .collect();

        match ret {
            LltsType::Void | LltsType::Never => {
                self.context.void_type().fn_type(&param_types, false)
            }
            _ => {
                let ret_ty = self.llvm_type(ret);
                ret_ty.fn_type(&param_types, false)
            }
        }
    }

    // ---- Compound type constructors ----

    /// String: `{ ptr, len }` — fat pointer to UTF-8 data.
    pub fn string_type(&self) -> StructType<'ctx> {
        let ptr_ty = self.context.ptr_type(AddressSpace::default()).into();
        let len_ty = self.context.i64_type().into();
        self.context.struct_type(&[ptr_ty, len_ty], false)
    }

    /// Named struct: `{ field1, field2, ... }`.
    pub fn struct_type(
        &mut self,
        name: &str,
        fields: &[(String, LltsType)],
    ) -> StructType<'ctx> {
        if let Some(&cached) = self.struct_cache.get(name) {
            return cached;
        }

        // Create an opaque struct first to allow recursive types.
        let opaque = self.context.opaque_struct_type(name);
        self.struct_cache.insert(name.to_string(), opaque);

        let field_types: Vec<BasicTypeEnum<'ctx>> = fields
            .iter()
            .map(|(_, ty)| self.llvm_type(ty))
            .collect();
        opaque.set_body(&field_types, false);

        opaque
    }

    /// Register an opaque struct type for forward declaration (pass 1).
    pub fn declare_struct(&mut self, name: &str) -> StructType<'ctx> {
        if let Some(&cached) = self.struct_cache.get(name) {
            return cached;
        }
        let opaque = self.context.opaque_struct_type(name);
        self.struct_cache.insert(name.to_string(), opaque);
        opaque
    }

    /// Define (set body of) a previously declared opaque struct.
    pub fn define_struct(
        &mut self,
        name: &str,
        fields: &[(String, LltsType)],
    ) -> StructType<'ctx> {
        let st = self.declare_struct(name);
        let field_types: Vec<BasicTypeEnum<'ctx>> = fields
            .iter()
            .map(|(_, ty)| self.llvm_type(ty))
            .collect();
        st.set_body(&field_types, false);
        st
    }

    /// Lookup a previously registered struct by name.
    pub fn get_struct(&self, name: &str) -> Option<StructType<'ctx>> {
        self.struct_cache.get(name).copied()
    }

    /// Array<T>: `{ ptr, len, cap }` — growable heap array.
    pub fn array_type(&mut self, elem: &LltsType) -> StructType<'ctx> {
        let _elem_ty = self.llvm_type(elem);
        let ptr_ty = self.context.ptr_type(AddressSpace::default()).into();
        let i64_ty = self.context.i64_type().into();
        self.context.struct_type(&[ptr_ty, i64_ty, i64_ty], false)
    }

    /// Option<T>: `{ i1, T }` — tag + value.
    pub fn option_type(&mut self, inner: &LltsType) -> StructType<'ctx> {
        let tag = self.context.bool_type().into();
        let val = self.llvm_type(inner);
        self.context.struct_type(&[tag, val], false)
    }

    /// Result<T, E>: `{ i32, max(sizeof(T), sizeof(E)) bytes }`.
    ///
    /// We represent the payload as a byte array sized to the larger variant,
    /// and bitcast when extracting. This avoids needing a true LLVM union.
    pub fn result_type(&mut self, ok: &LltsType, err: &LltsType) -> StructType<'ctx> {
        let tag = self.context.i32_type().into();
        let ok_ty = self.llvm_type(ok);
        let err_ty = self.llvm_type(err);
        // Use the larger of the two as the payload slot.
        let ok_size = self.type_size(ok);
        let err_size = self.type_size(err);
        let payload: BasicTypeEnum<'ctx> = if ok_size >= err_size { ok_ty } else { err_ty };
        self.context.struct_type(&[tag, payload], false)
    }

    /// Function value (fat pointer): `{ fn_ptr, env_ptr }`.
    pub fn fat_fn_type(
        &mut self,
        _params: &[LltsType],
        _ret: &LltsType,
    ) -> StructType<'ctx> {
        let ptr_ty = self.context.ptr_type(AddressSpace::default()).into();
        self.context.struct_type(&[ptr_ty, ptr_ty], false)
    }

    /// Tagged union: `{ i32, max_variant_bytes }`.
    pub fn union_type(
        &mut self,
        _name: &str,
        variants: &[(String, LltsType)],
    ) -> StructType<'ctx> {
        let tag = self.context.i32_type().into();
        let mut max_size: u64 = 0;
        let mut max_ty: BasicTypeEnum<'ctx> = self.context.i8_type().into();
        for (_, vty) in variants {
            let sz = self.type_size(vty);
            if sz > max_size {
                max_size = sz;
                max_ty = self.llvm_type(vty);
            }
        }
        self.context.struct_type(&[tag, max_ty], false)
    }

    /// Approximate size of a type in bytes (for union layout).
    fn type_size(&self, ty: &LltsType) -> u64 {
        match ty {
            LltsType::Bool => 1,
            LltsType::I8 | LltsType::U8 => 1,
            LltsType::I16 | LltsType::U16 => 2,
            LltsType::I32 | LltsType::U32 | LltsType::F32 => 4,
            LltsType::I64 | LltsType::U64 | LltsType::F64 => 8,
            LltsType::String | LltsType::Ptr => 16, // ptr + len or just ptr
            LltsType::Array(_) => 24,                // ptr + len + cap
            LltsType::Function { .. } => 16,         // fn_ptr + env_ptr
            LltsType::Option(_) => 16,               // conservative
            LltsType::Result { .. } => 16,            // conservative
            LltsType::Struct { fields, .. } => {
                fields.iter().map(|(_, f)| self.type_size(f)).sum()
            }
            LltsType::Union { variants, .. } => {
                4 + variants
                    .iter()
                    .map(|(_, v)| self.type_size(v))
                    .max()
                    .unwrap_or(0)
            }
            LltsType::Void | LltsType::Never => 0,
        }
    }

    /// Public static version of type_size for use outside the registry.
    pub fn type_size_of(ty: &LltsType) -> u64 {
        match ty {
            LltsType::Bool => 1,
            LltsType::I8 | LltsType::U8 => 1,
            LltsType::I16 | LltsType::U16 => 2,
            LltsType::I32 | LltsType::U32 | LltsType::F32 => 4,
            LltsType::I64 | LltsType::U64 | LltsType::F64 => 8,
            LltsType::String | LltsType::Ptr => 16,
            LltsType::Array(_) => 24,
            LltsType::Function { .. } => 16,
            LltsType::Option(_) => 16,
            LltsType::Result { .. } => 16,
            LltsType::Struct { fields, .. } => {
                fields.iter().map(|(_, f)| Self::type_size_of(f)).sum()
            }
            LltsType::Union { variants, .. } => {
                4 + variants
                    .iter()
                    .map(|(_, v)| Self::type_size_of(v))
                    .max()
                    .unwrap_or(0)
            }
            LltsType::Void | LltsType::Never => 0,
        }
    }

    pub fn context(&self) -> &'ctx Context {
        self.context
    }

    /// Return true if the type is a signed integer type.
    pub fn is_signed(ty: &LltsType) -> bool {
        matches!(ty, LltsType::I8 | LltsType::I16 | LltsType::I32 | LltsType::I64)
    }

    /// Return true if the type is an unsigned integer type.
    pub fn is_unsigned(ty: &LltsType) -> bool {
        matches!(ty, LltsType::U8 | LltsType::U16 | LltsType::U32 | LltsType::U64)
    }

    /// Return true if the type is any integer type.
    pub fn is_integer(ty: &LltsType) -> bool {
        Self::is_signed(ty) || Self::is_unsigned(ty)
    }

    /// Return true if the type is a floating-point type.
    pub fn is_float(ty: &LltsType) -> bool {
        matches!(ty, LltsType::F32 | LltsType::F64)
    }

    /// Return true if the type is numeric (integer or float).
    pub fn is_numeric(ty: &LltsType) -> bool {
        Self::is_integer(ty) || Self::is_float(ty)
    }

    /// Return the bit width of a numeric type.
    pub fn bit_width(ty: &LltsType) -> u32 {
        match ty {
            LltsType::I8 | LltsType::U8 | LltsType::Bool => 8,
            LltsType::I16 | LltsType::U16 => 16,
            LltsType::I32 | LltsType::U32 | LltsType::F32 => 32,
            LltsType::I64 | LltsType::U64 | LltsType::F64 => 64,
            _ => 0,
        }
    }
}
