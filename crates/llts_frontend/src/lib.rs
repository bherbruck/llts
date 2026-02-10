pub mod parse;
pub mod resolve;
pub mod semantic;

// Re-export key oxc types that downstream crates will need.
pub use oxc_allocator::Allocator;
pub use oxc_ast;
pub use oxc_semantic;
pub use oxc_span;
