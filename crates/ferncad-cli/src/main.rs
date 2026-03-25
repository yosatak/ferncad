/// ferncad CLI entry point
///
/// Usage:
///   ferncad <input.fern> --stl <output.stl>
///   ferncad <input.fern> --step <output.step>
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_usage();
        return;
    }

    if args[1] == "--version" || args[1] == "-V" {
        println!("ferncad v0.1.0");
        return;
    }

    let input_path = &args[1];
    let source = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read {input_path}: {e}");
            process::exit(1);
        }
    };

    // Parse export flags
    let mut stl_output: Option<String> = None;
    let mut step_output: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--stl" => {
                i += 1;
                stl_output = Some(args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("error: --stl requires an output path");
                    process::exit(1);
                }));
            }
            "--step" => {
                i += 1;
                step_output = Some(args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("error: --step requires an output path");
                    process::exit(1);
                }));
            }
            other => {
                eprintln!("error: unknown option: {other}");
                print_usage();
                process::exit(1);
            }
        }
        i += 1;
    }

    if stl_output.is_none() && step_output.is_none() {
        // Just evaluate and print result
        let mut evaluator = ferncad_core::evaluator::Evaluator::new();
        match evaluator.eval_source(&source) {
            Ok(value) => println!("{value}"),
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
        return;
    }

    if let Some(path) = stl_output {
        match ferncad_cad::realize::eval_and_realize(&source) {
            Ok(mesh) => {
                let bytes = ferncad_cad::export::export_stl_bytes(&mesh).unwrap_or_else(|e| {
                    eprintln!("error: STL export failed: {e}");
                    process::exit(1);
                });
                fs::write(&path, bytes).unwrap_or_else(|e| {
                    eprintln!("error: cannot write {path}: {e}");
                    process::exit(1);
                });
                eprintln!("STL exported: {path} ({} triangles)", mesh.triangle_count());
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
    }

    if let Some(path) = step_output {
        let mut evaluator = ferncad_core::evaluator::Evaluator::new();
        match evaluator.eval_source(&source) {
            Ok(ferncad_core::types::Value::Shape(node)) => {
                match ferncad_cad::step::export_step_bytes(&node) {
                    Ok(bytes) => {
                        fs::write(&path, &bytes).unwrap_or_else(|e| {
                            eprintln!("error: cannot write {path}: {e}");
                            process::exit(1);
                        });
                        eprintln!("STEP exported: {path} ({} bytes)", bytes.len());
                    }
                    Err(e) => {
                        eprintln!("error: STEP export failed: {e}");
                        process::exit(1);
                    }
                }
            }
            Ok(_) => {
                eprintln!("error: STEP export requires a shape expression");
                process::exit(1);
            }
            Err(e) => {
                eprintln!("error: {e}");
                process::exit(1);
            }
        }
    }
}

fn print_usage() {
    eprintln!("ferncad v0.1.0 — Lisp CAD modeler");
    eprintln!();
    eprintln!("Usage:");
    eprintln!("  ferncad <input.fern>                  Evaluate and print result");
    eprintln!("  ferncad <input.fern> --stl <out.stl>  Export as STL (triangulated mesh)");
    eprintln!("  ferncad <input.fern> --step <out.step> Export as STEP (exact BREP geometry)");
}
