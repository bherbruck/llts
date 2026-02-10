# Modules / Imports

## v1: Single Compilation Unit

All imports are resolved via oxc_resolver and inlined into one module. Produces one .o file, linked to one binary.

oxc_resolver handles the full complexity of TS/JS module resolution — tsconfig paths, package.json exports, extension resolution.

## v2: Per-Module Compilation

Per-module .o files for incremental builds. Compile each file separately, link together. Enables faster rebuilds when only one file changes.
