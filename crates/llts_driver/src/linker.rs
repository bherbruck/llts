use std::process::Command;

use crate::pipeline::CompileError;

/// Link an object file into a native binary using the system C compiler.
///
/// This invokes `cc` (or `gcc`/`clang` depending on the system) to link
/// the object file with libc, producing the final executable.
pub fn link(object_path: &str, output_path: &str) -> Result<(), CompileError> {
    let status = Command::new("cc")
        .arg(object_path)
        .arg("-o")
        .arg(output_path)
        .arg("-lm") // link libm for math functions
        .status()
        .map_err(|e| CompileError::Link(format!("failed to invoke linker: {e}")))?;

    if !status.success() {
        return Err(CompileError::Link(format!(
            "linker exited with status: {status}"
        )));
    }

    Ok(())
}
