use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveError, ResolveOptions, Resolution, Resolver};

/// A module resolver configured for TypeScript source files.
pub struct ModuleResolver {
    resolver: Resolver,
}

impl ModuleResolver {
    /// Create a new resolver with default options for LLTS TypeScript compilation.
    pub fn new() -> Self {
        Self {
            resolver: Resolver::new(ResolveOptions {
                extensions: vec![".ts".into(), ".tsx".into()],
                ..ResolveOptions::default()
            }),
        }
    }

    /// Create a resolver with custom options.
    pub fn with_options(options: ResolveOptions) -> Self {
        Self {
            resolver: Resolver::new(options),
        }
    }

    /// Resolve a module specifier relative to a directory.
    ///
    /// `directory` is the absolute path of the directory containing the importing file.
    /// `specifier` is the import string (e.g. `"./geometry"`, `"@scope/pkg"`).
    ///
    /// # Errors
    ///
    /// Returns `ResolveError` if the specifier cannot be resolved.
    pub fn resolve(&self, directory: &Path, specifier: &str) -> Result<Resolution, ResolveError> {
        self.resolver.resolve(directory, specifier)
    }

    /// Convenience: resolve a specifier relative to the parent directory of a source file.
    ///
    /// # Errors
    ///
    /// Returns `ResolveError` if the specifier cannot be resolved or if `source_file`
    /// has no parent directory.
    pub fn resolve_from_file(
        &self,
        source_file: &Path,
        specifier: &str,
    ) -> Result<Resolution, ResolveError> {
        let dir = source_file.parent().unwrap_or(Path::new("."));
        self.resolver.resolve(dir, specifier)
    }

    /// Get the resolved absolute path as a `PathBuf`, discarding query/fragment.
    pub fn resolve_to_path(
        &self,
        directory: &Path,
        specifier: &str,
    ) -> Result<PathBuf, ResolveError> {
        self.resolver
            .resolve(directory, specifier)
            .map(|r| r.into_path_buf())
    }
}

impl Default for ModuleResolver {
    fn default() -> Self {
        Self::new()
    }
}
