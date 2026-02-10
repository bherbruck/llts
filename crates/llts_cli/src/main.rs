use std::path::Path;
use std::process::{self, Command};

use clap::Parser;
use inkwell::OptimizationLevel;
use llts_driver::{compile_file, CompileOptions};

#[derive(Parser, Debug)]
#[command(name = "llts", about = "LLTS — TypeScript to native compiler")]
struct Cli {
    /// Input TypeScript file to compile.
    input: String,

    /// Output file path.
    #[arg(short, long)]
    output: Option<String>,

    /// Optimization level (0-3).
    #[arg(short = 'O', long = "opt-level", default_value = "0")]
    opt_level: u8,

    /// Emit LLVM IR text instead of a binary.
    #[arg(long)]
    emit_ir: bool,

    /// Compile and run immediately (binary is cleaned up after).
    #[arg(short, long)]
    run: bool,
}

fn main() {
    let cli = Cli::parse();

    let opt_level = match cli.opt_level {
        0 => OptimizationLevel::None,
        1 => OptimizationLevel::Less,
        2 => OptimizationLevel::Default,
        _ => OptimizationLevel::Aggressive,
    };

    // Determine output path: explicit -o, or temp file for --run, or build/<name>
    let stem = Path::new(&cli.input)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".to_string());

    let output = if let Some(ref o) = cli.output {
        o.clone()
    } else if cli.run {
        format!("/tmp/llts_run_{stem}_{}", process::id())
    } else {
        let build_dir = Path::new("build");
        if !build_dir.exists() {
            std::fs::create_dir_all(build_dir).unwrap_or_else(|e| {
                eprintln!("error: cannot create build directory: {e}");
                process::exit(1);
            });
        }
        format!("build/{stem}")
    };

    let options = CompileOptions {
        opt_level,
        emit_ir: cli.emit_ir,
        output: output.clone(),
    };

    let path = Path::new(&cli.input);
    if !path.exists() {
        eprintln!("error: file not found: {}", cli.input);
        process::exit(1);
    }

    match compile_file(path, &options) {
        Ok(()) => {
            if options.emit_ir {
                eprintln!("LLVM IR written to {output}");
            } else if cli.run {
                // Execute the compiled binary and forward its exit code
                let status = Command::new(&output)
                    .status()
                    .unwrap_or_else(|e| {
                        eprintln!("error: failed to run {output}: {e}");
                        process::exit(1);
                    });

                // Clean up temp binary unless user specified -o
                if cli.output.is_none() {
                    let _ = std::fs::remove_file(&output);
                }

                process::exit(status.code().unwrap_or(1));
            } else {
                eprintln!("compiled to {output}");
            }
        }
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    }
}
