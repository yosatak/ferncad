/// ferncad CLI entry point
///
/// Usage:
///   ferncad <input.fern> --stl <output.stl>
///   ferncad <input.fern> --step <output.step>
///   ferncad <input.fern> --segments <n>
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

    // Parse flags
    let mut stl_output: Option<String> = None;
    let mut step_output: Option<String> = None;
    let mut segments: Option<u32> = None;
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
            "--segments" => {
                i += 1;
                let val = args.get(i).cloned().unwrap_or_else(|| {
                    eprintln!("error: --segments requires a number");
                    process::exit(1);
                });
                segments = Some(val.parse::<u32>().unwrap_or_else(|_| {
                    eprintln!("error: --segments must be a positive integer, got: {val}");
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

    // Prepend *resolution* override if --segments was given
    let source = if let Some(seg) = segments {
        format!("(defvar *resolution* {seg})\n{source}")
    } else {
        source
    };

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
        match ferncad_cad::assembly_realize::eval_and_realize_parts(&source) {
            Ok(parts) if parts.len() > 1 => {
                // Assembly: export each part as <dir>/<stem>-<name>.stl
                let dir = std::path::Path::new(&path)
                    .parent()
                    .unwrap_or(std::path::Path::new("."));
                let stem = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("part");

                for part in &parts {
                    let part_path = dir.join(format!("{}-{}.stl", stem, part.name));
                    let bytes =
                        ferncad_cad::export::export_stl_bytes(&part.mesh).unwrap_or_else(|e| {
                            eprintln!("error: STL export failed for {}: {e}", part.name);
                            process::exit(1);
                        });
                    fs::write(&part_path, bytes).unwrap_or_else(|e| {
                        eprintln!("error: cannot write {}: {e}", part_path.display());
                        process::exit(1);
                    });
                    eprintln!(
                        "STL exported: {} ({} triangles)",
                        part_path.display(),
                        part.mesh.triangle_count()
                    );
                }
            }
            Ok(parts) if parts.len() == 1 => {
                let bytes =
                    ferncad_cad::export::export_stl_bytes(&parts[0].mesh).unwrap_or_else(|e| {
                        eprintln!("error: STL export failed: {e}");
                        process::exit(1);
                    });
                fs::write(&path, &bytes).unwrap_or_else(|e| {
                    eprintln!("error: cannot write {path}: {e}");
                    process::exit(1);
                });
                eprintln!(
                    "STL exported: {path} ({} triangles)",
                    parts[0].mesh.triangle_count()
                );
            }
            Ok(_) => {
                eprintln!("error: no shapes to export");
                process::exit(1);
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
            Ok(ferncad_core::types::Value::Shape(tracked)) => {
                match ferncad_cad::step::export_step_bytes(&tracked.node) {
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
    eprintln!("  ferncad <input.fern>                    Evaluate and print result");
    eprintln!(
        "  ferncad <input.fern> --stl <out.stl>    Export as STL (assembly → per-part files)"
    );
    eprintln!("  ferncad <input.fern> --step <out.step>  Export as STEP (exact BREP geometry)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --segments <n>  Set mesh resolution (default: 16, higher = smoother)");
    eprintln!("                  Can also be set in .fern: (defvar *resolution* 64)");
}
