use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../assets/branding/inconnu.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=RC");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../assets/branding/inconnu.ico")
        .canonicalize()
        .expect("Inconnu icon");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = version.split('-').next().unwrap().replace('.', ",") + ",0";
    let resource = format!(
        r#"1 ICON "{}"
1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x2L
FILEOS 0x40004L
FILETYPE 0x1L
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "CompanyName", "Frakd-Up Media\0"
   VALUE "FileDescription", "Inconnu - Character & Storyteller Utility\0"
   VALUE "FileVersion", "{version}\0"
   VALUE "InternalName", "Inconnu\0"
   VALUE "OriginalFilename", "Inconnu.exe\0"
   VALUE "ProductName", "Inconnu\0"
   VALUE "ProductVersion", "{version}\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x409, 1200
 END
END
"#,
        icon.to_string_lossy().replace('\\', "/")
    );
    let input = out.join("inconnu.rc");
    let output = out.join("inconnu.res");
    fs::write(&input, resource).expect("Write Windows resources");
    let compiler = env::var_os("RC").map(PathBuf::from).unwrap_or_else(|| {
        let kits = env::var_os("ProgramFiles(x86)")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"))
            .join("Windows Kits/10/bin");
        let mut versions = fs::read_dir(kits)
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("10."))
            })
            .collect::<Vec<_>>();
        versions.sort();
        versions
            .into_iter()
            .rev()
            .map(|p| p.join("x64/rc.exe"))
            .find(|p| p.is_file())
            .unwrap_or_else(|| PathBuf::from("rc.exe"))
    });
    let status = Command::new(compiler)
        .arg("/nologo")
        .arg("/fo")
        .arg(&output)
        .arg(&input)
        .status()
        .expect("Windows SDK resource compiler (rc.exe or RC override) is required");
    assert!(status.success(), "Windows resource compilation failed");
    println!("cargo:rustc-link-arg-bin=Inconnu={}", output.display());
}
