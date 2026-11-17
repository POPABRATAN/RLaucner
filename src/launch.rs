use crate::config::*;
use crate::mojang::*;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

fn subst(s: &str, m: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in m {
        out = out.replace(&format!("${{{k}}}"), v);
    }
    out
}

/// Разворачивает список аргументов из JSON (строки и объекты с rules).
fn expand_args(list: Option<&Value>, feats: &HashMap<String, bool>, m: &HashMap<&str, String>) -> Vec<String> {
    let mut out = vec![];
    for a in list.and_then(|l| l.as_array()).cloned().unwrap_or_default() {
        match &a {
            Value::String(s) => out.push(subst(s, m)),
            Value::Object(_) => {
                if !rules_allow(a.get("rules"), feats) {
                    continue;
                }
                match &a["value"] {
                    Value::String(s) => out.push(subst(s, m)),
                    Value::Array(arr) => {
                        for x in arr {
                            if let Some(s) = x.as_str() {
                                out.push(subst(s, m));
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

pub fn split_args(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for ch in s.chars() {
        match ch {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn build_command(root: &Path, profile: &Profile, acc: &Account, cfg: &Config, java: &Path, server: Option<&str>) -> Result<Command> {
    let v = resolve(root, &profile.version_id)?;
    let sep = if cfg!(windows) { ";" } else { ":" };
    let mut feats = default_features();
    feats.insert("is_quick_play_multiplayer".into(), server.is_some());

    // classpath (без дублей: побеждает первая запись — загрузчик)
    let mut seen = HashSet::new();
    let mut cp: Vec<String> = vec![];
    for lib in v["libraries"].as_array().cloned().unwrap_or_default() {
        if !rules_allow(lib.get("rules"), &feats) {
            continue;
        }
        if !seen.insert(lib_key(&lib)) {
            continue;
        }
        if let Some(f) = lib_main_file(&lib) {
            cp.push(root.join("libraries").join(&f.path).to_string_lossy().to_string());
        }
    }
    let client_id = v["_client_jar"].as_str().unwrap_or(&profile.version_id).to_string();
    let jar = root.join("versions").join(&client_id).join(format!("{client_id}.jar"));
    if jar.exists() {
        cp.push(jar.to_string_lossy().to_string());
    }
    let classpath = cp.join(sep);

    let game_dir = &profile.game_dir;
    std::fs::create_dir_all(game_dir)?;
    crate::install::prepare_legacy_assets(root, &v, game_dir)?;

    let assets_root = root.join("assets");
    let asset_id = v["assetIndex"]["id"].as_str().unwrap_or("legacy").to_string();
    let natives = root.join("versions").join(&profile.version_id).join("natives");
    let (utype, token) = match acc.kind {
        AccountKind::Microsoft => ("msa", acc.access_token.clone()),
        AccountKind::Offline => ("legacy", "0".to_string()),
    };
    let mut m: HashMap<&str, String> = HashMap::new();
    m.insert("auth_player_name", acc.name.clone());
    m.insert("auth_uuid", acc.uuid.clone());
    m.insert("auth_access_token", token);
    m.insert("auth_session", format!("token:0:{}", acc.uuid));
    m.insert("auth_xuid", "0".into());
    m.insert("clientid", "rlauncher".into());
    m.insert("user_type", utype.into());
    m.insert("user_properties", "{}".into());
    m.insert("version_name", profile.version_id.clone());
    m.insert("version_type", v["type"].as_str().unwrap_or("release").to_string());
    m.insert("game_directory", game_dir.to_string_lossy().to_string());
    m.insert("assets_root", assets_root.to_string_lossy().to_string());
    m.insert("game_assets", assets_root.join("virtual").join("legacy").to_string_lossy().to_string());
    m.insert("assets_index_name", asset_id);
    m.insert("natives_directory", natives.to_string_lossy().to_string());
    m.insert("launcher_name", "RLauncher".into());
    m.insert("launcher_version", APP_VERSION.into());
    m.insert("classpath", classpath.clone());
    m.insert("classpath_separator", sep.into());
    m.insert("library_directory", root.join("libraries").to_string_lossy().to_string());
    m.insert("resolution_width", cfg.width.to_string());
    m.insert("resolution_height", cfg.height.to_string());
    m.insert("quickPlayPath", root.join("logs").join("quickPlay.json").to_string_lossy().to_string());
    m.insert("quickPlayMultiplayer", server.unwrap_or("").to_string());
    m.insert("quickPlaySingleplayer", String::new());
    m.insert("quickPlayRealms", String::new());

    let modern = v.get("arguments").is_some();
    let mut jvm: Vec<String> = vec![format!("-Xmx{}M", cfg.memory_mb.max(512)), "-Dminecraft.launcher.brand=RLauncher".into(), format!("-Dminecraft.launcher.version={APP_VERSION}")];
    if modern {
        jvm.extend(expand_args(v["arguments"].get("jvm"), &feats, &m));
    } else {
        jvm.push(format!("-Djava.library.path={}", natives.display()));
    }
    if !jvm.iter().any(|a| a == "-cp" || a == "-classpath") {
        jvm.push("-cp".into());
        jvm.push(classpath);
    }
    if let Some(arg) = v["logging"]["client"]["argument"].as_str() {
        if let Some(id) = v["logging"]["client"]["file"]["id"].as_str() {
            let p = root.join("assets").join("log_configs").join(id);
            jvm.push(arg.replace("${path}", &p.to_string_lossy()));
        }
    }

    let mut game: Vec<String> = if modern {
        expand_args(v["arguments"].get("game"), &feats, &m)
    } else {
        v["minecraftArguments"].as_str().unwrap_or("").split_whitespace().map(|s| subst(s, &m)).collect()
    };
    if !modern {
        game.extend(["--width".into(), cfg.width.to_string(), "--height".into(), cfg.height.to_string()]);
        if let Some(s) = server {
            let (h, p) = s.split_once(':').unwrap_or((s, "25565"));
            game.extend(["--server".into(), h.into(), "--port".into(), p.into()]);
        }
    }
    if cfg.fullscreen {
        game.push("--fullscreen".into());
    }
    game.extend(split_args(&cfg.game_args));

    let main = v["mainClass"].as_str().ok_or_else(|| anyhow!("в описании версии нет mainClass"))?;
    let mut cmd = Command::new(java);
    cmd.args(&jvm).arg(main).args(&game).current_dir(game_dir);
    Ok(cmd)
}

pub fn log_path(root: &Path) -> PathBuf {
    root.join("logs").join("rlauncher-latest.log")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_quoted() {
        assert_eq!(split_args(r#"--a "b c" d"#), vec!["--a", "b c", "d"]);
    }
    #[test]
    fn offline_uuid_shape() {
        let u = crate::auth::offline_uuid("Steve");
        assert_eq!(u.len(), 32);
        assert_eq!(&u[12..13], "3");
    }
    #[test]
    fn rules() {
        let f = default_features();
        let r = serde_json::json!([{"action":"allow"},{"action":"disallow","os":{"name":"windows"}}]);
        assert_eq!(rules_allow(Some(&r), &f), !cfg!(windows));
        assert!(rules_allow(None, &f));
    }
}
