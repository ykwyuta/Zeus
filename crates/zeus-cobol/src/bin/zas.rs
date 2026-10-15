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
            "--listing",
            "--xref",
            "--listing-json",
            "--adata",
            "--adata-ut",
            "--adata-ccsid",
            "--source-listing",
            "--pcontrol",
            "--optable",
            "--compat",
            "--output-format",
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
    let mut output_format = None;
    let mut input = None;
    let mut link_inputs = Vec::new();
    let mut parm = String::new();
    let mut diagnostics = None;
    let mut listing = None;
    let mut cross_reference = None;
    let mut listing_json = None;
    let mut adata = None;
    let mut adata_ut = None;
    let mut adata_ccsid = None;
    let mut adata_rdw = false;
    let mut source_listing = None;
    let mut assembly_options = hlasm::AssemblyOptions::default();
    let mut has_sysparm = false;
    let mut has_source_encoding = false;
    let mut has_source_format = false;
    let mut has_pcontrol = false;
    let mut has_optable = false;
    let mut has_compat = false;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--adata" => {
                adata = Some(PathBuf::from(it.next().ok_or("--adata requires a file")?));
                assembly_options.collect_listing = true;
            }
            "--adata-ut" => {
                adata_ut = Some(
                    it.next()
                        .ok_or("--adata-ut requires an unsigned 64-bit UT value")?
                        .parse::<u64>()
                        .map_err(|_| "--adata-ut requires an unsigned 64-bit UT value")?,
                );
            }
            "--adata-ccsid" => {
                adata_ccsid = Some(
                    it.next()
                        .ok_or("--adata-ccsid requires a CCSID")?
                        .parse::<u16>()
                        .map_err(|_| "--adata-ccsid requires a 16-bit CCSID")?,
                );
            }
            "--adata-rdw" => adata_rdw = true,
            "--output-format" => {
                let format = it
                    .next()
                    .ok_or("--output-format requires zeus, obj or goff")?;
                if !matches!(format.as_str(), "zeus" | "obj" | "goff") {
                    return Err("--output-format requires zeus, obj or goff".into());
                }
                output_format = Some(format.as_str());
            }
            "--compat" => {
                has_compat = true;
                assembly_options.macrocase = Some(hlasm::parse_compat(
                    it.next().ok_or("--compat requires suboptions")?,
                )?);
            }
            "--optable" => {
                has_optable = true;
                assembly_options.optable = Some(hlasm::parse_optable(
                    it.next().ok_or("--optable requires a table")?,
                )?);
            }
            "--pcontrol" => {
                has_pcontrol = true;
                assembly_options.pcontrol_specified = true;
                assembly_options.pcontrol =
                    hlasm::parse_pcontrol(it.next().ok_or("--pcontrol requires options")?)?;
            }
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
            "--listing" | "--xref" | "--listing-json" | "--source-listing" => {
                let path = PathBuf::from(it.next().ok_or_else(|| format!("{a} requires a file"))?);
                match a.as_str() {
                    "--listing" => listing = Some(path),
                    "--xref" => cross_reference = Some(path),
                    "--source-listing" => source_listing = Some(path),
                    _ => listing_json = Some(path),
                }
                assembly_options.collect_listing = true;
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
    if adata.is_none() && (adata_ut.is_some() || adata_ccsid.is_some() || adata_rdw) {
        return Err("--adata-ut/--adata-ccsid/--adata-rdw require --adata".into());
    }
    if adata.is_some() && adata_ut.is_none() {
        return Err("--adata requires --adata-ut with the raw ADATA UT timestamp; automatic epoch conversion is not yet supported".into());
    }
    if adata.is_some() && !matches!(command, "compile" | "check") {
        return Err("--adata is available for compile/check".into());
    }
    if output_format.is_some() && !matches!(command, "compile" | "link") {
        return Err("--output-format is available for compile/link".into());
    }
    let output_format = output_format.unwrap_or("zeus");
    let extension = match output_format {
        "obj" => "obj",
        "goff" => "goff",
        _ => "zo",
    };
    if assembly_options.collect_listing && !matches!(command, "compile" | "check") {
        return Err(
            "--listing/--xref/--listing-json/--source-listing are available for compile/check"
                .into(),
        );
    }
    if diagnostics.is_some() && !matches!(command, "compile" | "check") {
        return Err("--diagnostics is available for compile/check".into());
    }
    if has_compat && !matches!(command, "compile" | "check") {
        return Err("--compat is available for compile/check".into());
    }
    if has_optable && !matches!(command, "compile" | "check") {
        return Err("--optable is available for compile/check".into());
    }
    if has_pcontrol && !matches!(command, "compile" | "check") {
        return Err("--pcontrol is available for compile/check".into());
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
        let output = out.ok_or("zas link requires -o OUTPUT")?;
        let module = hlasm::link_with_libraries(modules, &dirs)?;
        save_output(&module, &output, output_format)?;
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
            let path = out
                .clone()
                .unwrap_or_else(|| input.with_extension(extension));
            assembly_options.data_sets.insert(
                "SYSLIN".into(),
                hlasm::AssemblyDataSet {
                    name: path.display().to_string(),
                    ..Default::default()
                },
            );
        }
        if let Some(path) = &adata {
            assembly_options.data_sets.insert(
                "SYSADATA".into(),
                hlasm::AssemblyDataSet {
                    name: path.display().to_string(),
                    ..Default::default()
                },
            );
        }
        let report = hlasm::compile_report_with_options(&input, &dirs, &assembly_options);
        if let Some(path) = adata {
            let ccsid = adata_ccsid.unwrap_or(match assembly_options.source_encoding {
                hlasm::SourceEncoding::Ebcdic { ccsid, .. } => {
                    u16::try_from(ccsid).map_err(|_| "ADATA CCSID exceeds 16 bits")?
                }
                _ => 37,
            });
            let bytes = hlasm::adata_bytes(
                &report,
                &assembly_options,
                ccsid,
                adata_ut.unwrap(),
                adata_rdw,
            )?;
            std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        for diagnostic in &report.diagnostics {
            eprintln!("{diagnostic}");
        }
        if !report.complete {
            eprintln!("HLASM diagnostics are incomplete; no module will be emitted");
        }
        for (path, kind) in [(listing, 0), (cross_reference, 1), (source_listing, 2)] {
            if let Some(path) = path {
                let mut text =
                    report
                        .listing
                        .as_ref()
                        .map_or_else(String::new, |details| match kind {
                            1 => details.cross_reference(),
                            2 => details.original_source_text(),
                            _ => details.text(),
                        });
                text.push_str(&format!(
                    "\nAssembly complete: {}; return code: {}\n",
                    report.complete,
                    report.return_code()
                ));
                for diagnostic in &report.diagnostics {
                    text.push_str(&format!("{diagnostic}\n"));
                }
                std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
            }
        }
        if let Some(path) = listing_json {
            let bytes = serde_json::to_vec_pretty(&report.listing).map_err(|e| e.to_string())?;
            std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        if let Some(path) = diagnostics {
            let bytes = serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?;
            std::fs::write(path, bytes).map_err(|e| e.to_string())?;
        }
        if command == "compile"
            && let Some(module) = &report.module
        {
            save_output(
                module,
                &out.unwrap_or_else(|| input.with_extension(extension)),
                output_format,
            )?;
        }
        Ok(i32::from(report.return_code()))
    }
}

fn save_output(module: &hlasm::Module, path: &std::path::Path, format: &str) -> Result<(), String> {
    // Encode completely before touching an existing output file.
    let bytes = match format {
        "obj" => hlasm::object_bytes(module)?,
        "goff" => hlasm::goff_bytes(module)?,
        _ => hlasm::module_bytes(module)?,
    };
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}
