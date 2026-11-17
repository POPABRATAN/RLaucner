use crate::config::Loader;
use crate::java::ensure_java;
use crate::mojang::{java_requirement, resolve};
use crate::net::*;
use anyhow::{anyhow, bail, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;

fn xml_versions(xml: &str) -> Vec<String> {
    xml.split("<version>")
        .skip(1)
        .filter_map(|s| s.split("</version>").next())
        .map(|s| s.trim().to_string())
        .collect()
}

/// Версии загрузчика для данной версии Minecraft, новые — первыми.
pub async fn loader_versions(c: &reqwest::Client, loader: Loader, mc: &str) -> Result<Vec<String>> {
    Ok(match loader {
        Loader::Vanilla => vec![],
        Loader::Fabric | Loader::Quilt => {
            let base = if loader == Loader::Fabric {
                "https://meta.fabricmc.net/v2/versions/loader"
            } else {
                "https://meta.quiltmc.org/v3/versions/loader"
            };
            let arr: Vec<Value> = get_json(c, &format!("{base}/{mc}")).await?;
            arr.iter().filter_map(|e| e["loader"]["version"].as_str().map(String::from)).collect()
        }
        Loader::Forge => {
            let xml = get_text(c, "https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml").await?;
            let prefix = format!("{mc}-");
            let mut v: Vec<String> = xml_versions(&xml)
                .into_iter()
                .filter(|s| s.starts_with(&prefix))
                .map(|s| s[prefix.len()..].to_string())
                .collect();
            v.reverse();
            v
        }
        Loader::NeoForge => {
            let xml = get_text(c, "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml").await?;
            let prefix = neoforge_prefix(mc);
            let mut v: Vec<String> = xml_versions(&xml).into_iter().filter(|s| s.starts_with(&prefix)).collect();
            v.reverse();
            v
        }
    })
}

fn neoforge_prefix(mc: &str) -> String {
    // 1.21.1 -> "21.1.", 1.20.4 -> "20.4.", 1.21 -> "21.0."
    if let Some(rest) = mc.strip_prefix("1.") {
        let mut it = rest.split('.');
        let minor = it.next().unwrap_or("0");
        let patch = it.next().unwrap_or("0");
        format!("{minor}.{patch}.")
    } else {
        format!("{mc}.")
    }
}

/// Устанавливает загрузчик и возвращает id версии для запуска.
pub async fn install_loader(core: &Core, loader: Loader, mc: &str, lv: &str) -> Result<String> {
    match loader {
        Loader::Vanilla => Ok(mc.to_string()),
        Loader::Fabric | Loader::Quilt => {
            let url = if loader == Loader::Fabric {
                format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{lv}/profile/json")
            } else {
                format!("https://meta.quiltmc.org/v3/versions/loader/{mc}/{lv}/profile/json")
            };
            set_prog(&core.prog, format!("Установка {}", loader.short()), 0, 1);
            let v: Value = get_json(&core.http, &url).await?;
            let id = v["id"].as_str().ok_or_else(|| anyhow!("в профиле загрузчика нет id"))?.to_string();
            let p = crate::mojang::version_json_path(&core.root, &id);
            std::fs::create_dir_all(p.parent().unwrap())?;
            std::fs::write(&p, serde_json::to_vec_pretty(&v)?)?;
            Ok(id)
        }
        Loader::Forge => {
            let full = format!("{mc}-{lv}");
            let url = format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar");
            run_installer(core, &url, &format!("forge-{full}-installer.jar"), mc, &format!("{mc}-forge-{lv}")).await
        }
        Loader::NeoForge => {
            let url = format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar");
            run_installer(core, &url, &format!("neoforge-{lv}-installer.jar"), mc, &format!("neoforge-{lv}")).await
        }
    }
}

fn list_versions(root: &Path) -> HashSet<String> {
    std::fs::read_dir(root.join("versions"))
        .map(|rd| rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().to_string()).collect())
        .unwrap_or_default()
}

async fn run_installer(core: &Core, url: &str, file: &str, mc: &str, expected_id: &str) -> Result<String> {
    // уже установлено?
    if crate::mojang::version_json_path(&core.root, expected_id).exists() {
        return Ok(expected_id.to_string());
    }
    let jar = core.root.join("installers").join(file);
    set_prog(&core.prog, "Загрузка установщика загрузчика", 0, 1);
    download(&core.http, url, &jar, None, None, false).await?;

    let vjson = resolve(&core.root, mc)?;
    let (component, _) = java_requirement(&vjson);
    let java = ensure_java(core, &component, "", true).await?;

    // установщику нужен launcher_profiles.json
    let lp = core.root.join("launcher_profiles.json");
    if !lp.exists() {
        std::fs::create_dir_all(&core.root)?;
        std::fs::write(&lp, br#"{"profiles":{},"version":3}"#)?;
    }
    let before = list_versions(&core.root);
    set_prog(&core.prog, "Работает установщик загрузчика (это может занять несколько минут)…", 0, 0);
    let mut cmd = tokio::process::Command::new(&java);
    cmd.arg("-jar").arg(&jar).arg("--installClient").arg(&core.root).current_dir(&core.root);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    let out = cmd.output().await.map_err(|e| anyhow!("не удалось запустить установщик: {e}"))?;
    if !out.status.success() {
        let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = text.lines().rev().take(6).collect();
        bail!("Установщик завершился с ошибкой:\n{}", tail.into_iter().rev().collect::<Vec<_>>().join("\n"));
    }
    if crate::mojang::version_json_path(&core.root, expected_id).exists() {
        return Ok(expected_id.to_string());
    }
    let after = list_versions(&core.root);
    after
        .difference(&before)
        .next()
        .cloned()
        .ok_or_else(|| anyhow!("установщик отработал, но новая версия не найдена"))
}
