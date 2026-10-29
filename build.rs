use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn find_rc() -> Option<PathBuf> {
    if let Ok(rc) = env::var("RC") {
        let path = PathBuf::from(rc);
        if path.is_file() {
            return Some(path);
        }
    }

    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let mut roots = Vec::new();
    if let Ok(sdk) = env::var("WindowsSdkDir") {
        roots.push(PathBuf::from(sdk).join("bin"));
    }
    roots.push(PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10\bin"));

    let mut versions = Vec::new();
    for root in roots {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let candidate = dir.join("x64").join("rc.exe");
            if candidate.is_file() {
                versions.push((entry.file_name().to_string_lossy().into_owned(), candidate));
            }
        }
    }
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().next().map(|(_, path)| path)
}

fn main() {
    println!("cargo:rerun-if-changed=assets/neno.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let icon = manifest.join("assets").join("neno.ico");
    if !icon.is_file() {
        return;
    }

    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found; building without an executable icon");
        return;
    };

    let out = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    let script = out.join("neno_icon.rc");
    let resource = out.join("neno_icon.res");
    let icon_path = icon.display().to_string().replace('\\', "\\\\");
    if let Err(err) = fs::write(&script, format!("1 ICON \"{icon_path}\"\n")) {
        println!("cargo:warning=could not write icon resource script: {err}");
        return;
    }

    let status = Command::new(rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg(&script)
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bins={}", resource.display());
        }
        Ok(status) => println!("cargo:warning=rc.exe failed with {status}; building without an executable icon"),
        Err(err) => println!("cargo:warning=could not run rc.exe: {err}; building without an executable icon"),
    }
}
