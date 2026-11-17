use crate::mojang::*;
use crate::net::*;
use anyhow::{anyhow, bail, Result};
use futures::future::BoxFuture;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Устанавливает версию: описание, клиент, библиотеки, natives, ассеты, лог-конфиг.
pub fn install_version<'a>(core: &'a Core, id: &'a str) -> BoxFuture<'a, Result<()>> {
    Box::pin(async move {
        let root = &core.root;
        let jpath = version_json_path(root, id);
        if !jpath.exists() || core.force {
            if let Some(mv) = core.manifest.versions.iter().find(|v| v.id == id) {
                set_prog(&core.prog, format!("Описание версии {id}"), 0, 1);
                if core.force {
                    let _ = std::fs::remove_file(&jpath);
                }
                download(&core.http, &mv.url, &jpath, None, None, false).await?;
            } else if !jpath.exists() {
                bail!("Версия {id} не найдена (нет в списке Mojang и не установлена)");
            }
        }
        let raw = read_json(&jpath)?;
        let inherits = raw["inheritsFrom"].as_str().map(String::from);
        if let Some(parent) = &inherits {
            install_version(core, parent).await?;
        }
        let v = resolve(root, id)?;

        // клиент (только у «настоящих» версий Mojang)
        if inherits.is_none() {
            if let Some(c) = v["downloads"].get("client") {
                let dest = root.join("versions").join(id).join(format!("{id}.jar"));
                run_downloads(
                    core,
                    vec![Task {
                        url: c["url"].as_str().unwrap_or("").into(),
                        dest,
                        sha1: c["sha1"].as_str().map(String::from),
                        size: c["size"].as_u64(),
                    }],
                    "Загрузка клиента Minecraft",
                )
                .await?;
            }
        }

        // библиотеки
        let feats = default_features();
        let mut tasks = vec![];
        let mut natives: Vec<PathBuf> = vec![];
        for lib in v["libraries"].as_array().cloned().unwrap_or_default() {
            if !rules_allow(lib.get("rules"), &feats) {
                continue;
            }
            if let Some(f) = lib_main_file(&lib) {
                let dest = root.join("libraries").join(&f.path);
                if !f.url.is_empty() {
                    tasks.push(Task { url: f.url, dest: dest.clone(), sha1: f.sha1, size: f.size });
                }
                if lib_is_natives_jar(&lib) {
                    natives.push(dest);
                }
            }
            if let Some(f) = lib_native_file(&lib) {
                let dest = root.join("libraries").join(&f.path);
                tasks.push(Task { url: f.url, dest: dest.clone(), sha1: f.sha1, size: f.size });
                natives.push(dest);
            }
        }
        run_downloads(core, tasks, "Загрузка библиотек").await?;

        let ndir = root.join("versions").join(id).join("natives");
        let marker = ndir.join(".done");
        if core.force || !marker.exists() {
            std::fs::create_dir_all(&ndir)?;
            for j in &natives {
                if j.exists() {
                    let _ = extract_natives(j, &ndir);
                }
            }
            std::fs::write(&marker, b"ok")?;
        }

        install_assets(core, &v).await?;

        // конфиг логирования (закрывает уязвимость log4j в старых версиях)
        if let Some(f) = v["logging"]["client"]["file"].as_object() {
            if let (Some(fid), Some(url)) = (f.get("id").and_then(|x| x.as_str()), f.get("url").and_then(|x| x.as_str())) {
                let dest = root.join("assets").join("log_configs").join(fid);
                download(&core.http, url, &dest, f.get("sha1").and_then(|x| x.as_str()), f.get("size").and_then(|x| x.as_u64()), false).await?;
            }
        }
        Ok(())
    })
}

fn extract_natives(jar: &Path, dir: &Path) -> Result<()> {
    let f = std::fs::File::open(jar)?;
    let mut z = zip::ZipArchive::new(f)?;
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        let lower = name.to_lowercase();
        if name.starts_with("META-INF") || !(lower.ends_with(".dll") || lower.ends_with(".so") || lower.ends_with(".dylib") || lower.ends_with(".jnilib")) {
            continue;
        }
        let fname = Path::new(&name).file_name().ok_or_else(|| anyhow!("bad entry"))?;
        let mut buf = vec![];
        e.read_to_end(&mut buf)?;
        std::fs::write(dir.join(fname), buf)?;
    }
    Ok(())
}

async fn install_assets(core: &Core, v: &Value) -> Result<()> {
    let ai = &v["assetIndex"];
    let (Some(id), Some(url)) = (ai["id"].as_str(), ai["url"].as_str()) else {
        return Ok(());
    };
    let root = &core.root;
    let idx_path = root.join("assets").join("indexes").join(format!("{id}.json"));
    download(&core.http, url, &idx_path, ai["sha1"].as_str(), ai["size"].as_u64(), false).await?;
    let idx = read_json(&idx_path)?;
    let mut tasks = vec![];
    if let Some(objs) = idx["objects"].as_object() {
        for (_, o) in objs {
            let Some(h) = o["hash"].as_str() else { continue };
            let p2 = &h[..2.min(h.len())];
            tasks.push(Task {
                url: format!("https://resources.download.minecraft.net/{p2}/{h}"),
                dest: root.join("assets").join("objects").join(p2).join(h),
                sha1: Some(h.to_string()),
                size: o["size"].as_u64(),
            });
        }
    }
    run_downloads(core, tasks, "Загрузка ресурсов игры").await
}

/// Для очень старых версий: копирует ассеты в virtual/legacy или resources/.
pub fn prepare_legacy_assets(root: &Path, v: &Value, game_dir: &Path) -> Result<()> {
    let Some(id) = v["assetIndex"]["id"].as_str() else { return Ok(()) };
    let idx = read_json(&root.join("assets").join("indexes").join(format!("{id}.json")))?;
    let virt = idx["virtual"].as_bool().unwrap_or(false);
    let map = idx["map_to_resources"].as_bool().unwrap_or(false);
    if !virt && !map {
        return Ok(());
    }
    let base = if map { game_dir.join("resources") } else { root.join("assets").join("virtual").join("legacy") };
    if let Some(objs) = idx["objects"].as_object() {
        for (name, o) in objs {
            let Some(h) = o["hash"].as_str() else { continue };
            let src = root.join("assets").join("objects").join(&h[..2]).join(h);
            let dst = base.join(name);
            if dst.exists() || !src.exists() {
                continue;
            }
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}
