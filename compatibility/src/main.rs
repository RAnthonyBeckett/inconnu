use grapevine_legacy_reader::{MAX_INPUT_BYTES, inspect};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

fn main() {
    match run() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Reader error: {e}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<i32, Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut input = None;
    let mut output = None;
    let mut html_output = None;
    let mut encoding = String::from("windows-1252");
    let mut json_stdout = false;
    let mut list = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => {
                println!(
                    "Grapevine Legacy Reader 0.1.0\n\nUsage: grapevine-legacy-reader FILE [--output-file NEW.json] [--html-file NEW.html] [--list] [--encoding windows-1252] [--json]\n\nReads GVBG/GVBE binary and Grapevine XML. Never modifies input or overwrites an existing output.\n--list prints record names. --html-file writes a readable, standalone private inventory.\nExit codes: 0 complete structural read, 2 partial/unsupported/invalid file, 1 command or I/O error.\nReports can contain player contact details and ST secrets. Keep them private.\nThis is an inspector, not a finished character importer, rules engine or legacy file writer."
                );
                return Ok(0);
            }
            Some("--version") => {
                println!("grapevine-legacy-reader 0.1.0");
                return Ok(0);
            }
            Some("--output-file") => {
                if output.is_some() {
                    return Err("Duplicate --output-file".into());
                }
                output = Some(PathBuf::from(args.next().ok_or("Missing output path")?));
            }
            Some("--html-file") => {
                if html_output.is_some() {
                    return Err("Duplicate --html-file".into());
                }
                html_output = Some(PathBuf::from(
                    args.next().ok_or("Missing HTML output path")?,
                ));
            }
            Some("--encoding") => {
                encoding = args
                    .next()
                    .ok_or("Missing encoding")?
                    .into_string()
                    .map_err(|_| "Invalid encoding label")?
            }
            Some("--json") => json_stdout = true,
            Some("--list") => list = true,
            Some(s) if s.starts_with('-') => return Err(format!("Unknown option {s}").into()),
            _ => {
                if input.is_some() {
                    return Err("Supply one input file at a time".into());
                }
                input = Some(PathBuf::from(arg));
            }
        }
    }
    let input = input.ok_or("Supply a .gex or .gv3 file; use --help for details")?;
    let file = File::open(&input)?;
    if !file.metadata()?.is_file() {
        return Err("Input must be a regular file".into());
    }
    if file.metadata()?.len() > MAX_INPUT_BYTES as u64 {
        return Err("Input exceeds the 32 MiB limit".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err("Input grew beyond the 32 MiB limit".into());
    }
    let report = inspect(&bytes, &encoding);
    let serialized = if output.is_some() || json_stdout {
        Some(serde_json::to_vec_pretty(&report)?)
    } else {
        None
    };
    if let Some(output) = output {
        // create_new prevents clobbering sources, reports and hard-link aliases.
        let mut dest = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        dest.write_all(serialized.as_ref().unwrap())?;
        dest.write_all(b"\n")?;
        dest.sync_all()?;
        eprintln!("Private inspection report saved to {}", output.display());
    }
    if let Some(output) = html_output {
        let html = grapevine_legacy_reader::html::render(
            &report,
            &input.file_name().unwrap_or_default().to_string_lossy(),
        );
        let mut dest = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)?;
        dest.write_all(html.as_bytes())?;
        dest.sync_all()?;
        eprintln!("Private readable inventory saved to {}", output.display());
    }
    if json_stdout {
        std::io::stdout().write_all(serialized.as_ref().unwrap())?;
        println!();
    } else {
        println!(
            "{}\nFormat: {} | version: {:?}\nResult: {} | {} records | {} bytes",
            input.display(),
            report.format,
            report.version,
            report.status,
            report.record_count(),
            report.input_bytes
        );
        for (name, records) in &report.sections {
            println!("  {name}: {}", records.len());
            if list {
                for record in records {
                    println!(
                        "    {}",
                        grapevine_legacy_reader::html::record_name(record)
                            .replace(['\r', '\n'], " ")
                    );
                }
            }
        }
        for d in &report.diagnostics {
            if d.severity != "info" {
                println!(
                    "  {} [{}] {} at {} {:?}",
                    d.severity, d.code, d.message, d.path, d.offset
                );
            }
        }
        println!("Input SHA-256: {}", report.sha256);
    }
    Ok(if report.succeeded() { 0 } else { 2 })
}
