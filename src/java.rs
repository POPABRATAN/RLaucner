use crate::net::*;
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

const ALL_URL: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

fn platform_key() -> &'static str {
    if cfg!(windows) {
        if cfg!(target_arch = "aarch64") { "windows-arm64" } else { "windows-x64" }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { "mac-os-arm64" } else { "mac-os" }
    } else {
        "linux"
    }
}

pub fn java_bin(dir: &Path, console: bool) -> PathBuf {
    if cfg!(windows) {
        dir.join("bin").join(if console { "java.exe" } else { "javaw.exe" })
    } else if cfg!(target_os = "macos") {
        let p = dir.join("jre.bundle/Contents/Home/bin/java");
        if p.exists() { p } else { dir.join("bin/java") }
    } else {
        dir.join("bin/java")
    }
}

/// Возвращает путь к Java. Если пользователь указал свой путь — берёт его,
/// иначе скачивает официальную среду выполнения Mojang нужной версии.
pub async fn ensure_java(core: &Core, component: &str, custom: &str, console: bool) -> Result<PathBuf> {
    if !custom.trim().is_empty() {
        let p = PathBuf::from(custom.trim());
        if !p.exists() {
            bail!("Указанный путь к Java не существует: {}", p.display());
        }
        return Ok(p);
    }
    let dir = core.root.join("runtime").join(component);
    let marker = dir.join(".rl_ok");
    if marker.exists() {
        return Ok(java_bin(&dir, console));
    }
    set_prog(&core.prog, format!("Загрузка Java ({component})"), 0, 1);
    let all: Value = get_json(&core.http, ALL_URL).await?;
    let manifest_url = all[platform_key()][component][0]["manifest"]["url"]
        .as_str()
        .ok_or_else(|| anyhow!("Mojang не предоставляет Java «{component}» для этой системы. Укажите свою Java в настройках."))?
        .to_string();
    let manifest: Value = get_json(&core.http, &manifest_url).await?;
    let mut tasks = vec![];
    let mut exec: Vec<PathBuf> = vec![];
    if let Some(files) = manifest["files"].as_object() {
        for (rel, f) in files {
            let dest = dir.join(rel);
            match f["type"].as_str() {
                Some("directory") => {
                    std::fs::create_dir_all(&dest)?;
                }
                Some("file") => {
                    let raw = &f["downloads"]["raw"];
                    tasks.push(Task {
                        url: raw["url"].as_str().unwrap_or("").into(),
                        dest: dest.clone(),
                        sha1: raw["sha1"].as_str().map(String::from),
                        size: raw["size"].as_u64(),
                    });
                    if f["executable"].as_bool().unwrap_or(false) {
                        exec.push(dest);
                    }
                }
                _ => {}
            }
        }
    }
    run_downloads(core, tasks, &format!("Загрузка Java ({component})")).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in exec {
            let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
        }
    }
    #[cfg(not(unix))]
    let _ = exec;
    std::fs::write(&marker, b"ok")?;
    Ok(java_bin(&dir, console))
}
