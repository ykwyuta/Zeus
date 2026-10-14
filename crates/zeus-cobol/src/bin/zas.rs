//! Application assembler compiler, linker and runner.
use std::path::PathBuf;
use std::process::ExitCode;
use zeus_cobol::{hlasm, vm::Machine};

#[path = "../../../cli_help.rs"]
mod cli_help;

fn main() -> ExitCode {
    if cli_help::show(
        include_str!("../../../../docs/help/zas.txt"),
        &[
            "-I",
            "-L",
            "-o",
            "--parm",
            "--sysparm",
            "--diagnostics",
            "--source-encoding",
            "--source-format",
        ],
    ) {
        return ExitCode::SUCCESS;
    }
    match run() {
        Ok(rc) => ExitCode::from((rc & 255) as u8),
        Err(e) => {
            eprintln!("zas: {e}");
            ExitCode::from(12)
        }
    }
}

fn run() -> Result<i32, String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut it = args.iter();
    let command = it.next().map(String::as_str).unwrap_or("");
    if command == "--version" {
        println!("zas {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if !matches!(command, "compile" | "check" | "link" | "run") {
        return Err("use zas compile|check|link|run; see zas --help".into());
    }
    let mut dirs = Vec::new();
    let mut out = None;
    let mut input = None;
    let mut link_inputs = Vec::new();
    let mut parm = String::new();
    let mut diagnostics = None;
    let mut assembly_options = hlasm::AssemblyOptions::default();
    let mut has_sysparm = false;
    let mut has_source_encoding = false;
    let mut has_source_format = false;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--source-format" => {
                has_source_format = true;
                assembly_options.source_format = it
                    .next()
                    .ok_or("--source-format requires a value")?
                    .parse()?;
            }
            "--source-encoding" => {
                has_source_encoding = true;
                assembly_options.source_encoding = it
                    .next()
                    .ok_or("--source-encoding requires a value")?
                    .parse()?;
            }
            "-I" | "-L" => dirs.push(PathBuf::from(
                it.next().ok_or("-I/-L requires a directory")?,
            )),
            "-o" => out = Some(PathBuf::from(it.next().ok_or("-o requires a file")?)),
            "--parm" => parm = it.next().ok_or("--parm requires text")?.clone(),
            "--sysparm" => {
                has_sysparm = true;
                assembly_options.sysparm = it.next().ok_or("--sysparm requires text")?.clone()
            }
            "--diagnostics" => {
                diagnostics = Some(PathBuf::from(
                    it.next().ok_or("--diagnostics requires a JSON file")?,
                ))
            }
            s if s.starts_with('-') => return Err(format!("unknown option {s}")),
            _ => {
                if command == "link" {
                    link_inputs.push(PathBuf::from(a));
                    continue;
                }
                if input.replace(PathBuf::from(a)).is_some() {
                    return Err("one input file required".into());
                }
            }
        }
    }
    if diagnostics.is_some() && !matches!(command, "compile" | "check") {
        return Err("--diagnostics is available for compile/check".into());
    }
    if has_sysparm && !matches!(command, "compile" | "check") {
        return Err("--sysparm is available for compile/check".into());
    }
    if has_source_encoding && !matches!(command, "compile" | "check") {
        return Err("--source-encoding is available for compile/check".into());
    }
    if has_source_format && !matches!(command, "compile" | "check") {
        return Err("--source-format is available for compile/check".into());
    }
    if command == "link" {
        let mut modules = Vec::new();
        for path in link_inputs {
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            modules.push(hlasm::load_module(&bytes)?);
        }
        let output = out.ok_or("zas link requires -o OUTPUT.zo")?;
        let module = hlasm::link_with_libraries(modules, &dirs)?;
        hlasm::save_module(&module, &output)?;
        return Ok(0);
    }
    let input = input.ok_or("input file required; see zas --help")?;
    if command == "run" {
        let bytes = std::fs::read(&input).map_err(|e| e.to_string())?;
        let module = hlasm::load_module(&bytes)?;
        let mut libraries = vec![
            input
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf(),
        ];
        libraries.extend(dirs);
        let mut machine = Machine::new(libraries);
        machine.set_step(zeus_cobol::vm::StepInfo {
            parm,
            ..Default::default()
        });
        machine.run_hlasm_main(module)
    } else {
        let mut assembly_options = assembly_options;
        if command == "compile" {
            let path = out.clone().unwrap_or_else(|| input.with_extension("zo"));
            assembly_options.data_sets.insert(
                "SYSLIN".into(),
                hlasm::AssemblyDataSet {
                    name: path.display().to_string(),
                    ..Default::default()
                },
            );
        }
        let report = hlasm::compile_report_with_options(&input, &dirs, &assembly_options);
        for diagnostic in &report.diagnostics {
            eprintln!("{diagnostic}");
        }
        if !report.complete {
            eprintln!("HLASM diagnostics are incomplete; no module will be emitted");
        }
        if let Some(path) = diagnostics {
            let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
            std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
        if command == "compile"
            && let Some(module) = &report.module
        {
            hlasm::save_module(module, &out.unwrap_or_else(|| input.with_extension("zo")))?;
        }
        Ok(i32::from(report.return_code()))
    }
}
