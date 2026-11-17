use std::{
    backtrace::Backtrace,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn directory() -> PathBuf {
    std::env::var_os("INCONNU_DIAGNOSTICS_DIR")
        .or_else(|| std::env::var_os("GRAPEVINE_DIAGNOSTICS_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join("Inconnu/Diagnostics")
        })
}

pub fn write(dir: &Path, operation: &str, code: &str, trace: bool) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;
    let safe = |s: &str| {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
            .take(80)
            .collect::<String>()
    };
    let event = serde_json::json!({"unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),"build":env!("CARGO_PKG_VERSION"),"operation":safe(operation),"code":safe(code),"backtrace":if trace {Some(Backtrace::force_capture().to_string())} else {None}});
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(dir.join("events.jsonl"))?;
    writeln!(file, "{event}")?;
    file.sync_data()
}

pub fn record(operation: &str, code: &str) {
    let _ = write(&directory(), operation, code, code != "OK");
}
pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|_| {
        let _ = write(&directory(), "panic", "APP_PANIC", true);
    }));
}
