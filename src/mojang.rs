use crate::net::*;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Deserialize, Clone, Debug)]
pub struct ManifestVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
}
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Latest {
    pub release: String,
    pub snapshot: String,
}
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Manifest {
    pub latest: Latest,
    pub versions: Vec<ManifestVersion>,
}

pub async fn fetch_manifest(c: &reqwest::Client) -> Result<Manifest> {
    let cache = crate::config::launcher_dir().join("version_manifest.json");
    match get_text(c, MANIFEST_URL).await {
        Ok(t) => {
            let m: Manifest = serde_json::from_str(&t)?;
            let _ = std::fs::create_dir_all(crate::config::launcher_dir());
            let _ = std::fs::write(&cache, t);
            Ok(m)
        }
        Err(e) => {
            let t = std::fs::read_to_string(&cache)
                .map_err(|_| anyhow!("нет связи с серверами Mojang и нет кэша: {e}"))?;
            Ok(serde_json::from_str(&t)?)
        }
    }
}

pub fn os_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}

fn arch_matches(a: &str) -> bool {
    match a {
        "x86" => cfg!(target_arch = "x86"),
        "x86_64" | "amd64" => cfg!(target_arch = "x86_64"),
        "arm64" | "aarch64" => cfg!(target_arch = "aarch64"),
        _ => false,
    }
}

pub fn rules_allow(rules: Option<&Value>, feats: &HashMap<String, bool>) -> bool {
    let Some(arr) = rules.and_then(|r| r.as_array()) else {
        return true;
    };
    if arr.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in arr {
        let allow = r["action"].as_str().unwrap_or("allow") == "allow";
        let mut m = true;
        if let Some(os) = r.get("os") {
            if let Some(n) = os["name"].as_str() {
                if n != os_name() {
                    m = false;
                }
            }
            if let Some(a) = os["arch"].as_str() {
                if !arch_matches(a) {
                    m = false;
                }
            }
        }
        if let Some(f) = r.get("features").and_then(|f| f.as_object()) {
            for (k, v) in f {
                if feats.get(k).copied().unwrap_or(false) != v.as_bool().unwrap_or(false) {
                    m = false;
                }
            }
        }
        if m {
            allowed = allow;
        }
    }
    allowed
}

pub fn read_json(p: &Path) -> Result<Value> {
    let s = std::fs::read_to_string(p).map_err(|e| anyhow!("{}: {e}", p.display()))?;
    Ok(serde_json::from_str(&s)?)
}

pub fn version_json_path(root: &Path, id: &str) -> std::path::PathBuf {
    root.join("versions").join(id).join(format!("{id}.json"))
}

/// Читает версию и склеивает её с родителем (inheritsFrom).
pub fn resolve(root: &Path, id: &str) -> Result<Value> {
    let v = read_json(&version_json_path(root, id))?;
    if let Some(parent) = v["inheritsFrom"].as_str() {
        let p = resolve(root, parent)?;
        return Ok(merge(p, v));
    }
    Ok(v)
}

fn merge(parent: Value, child: Value) -> Value {
    let mut out = parent.clone();
    let o = out.as_object_mut().unwrap();
    if let Some(c) = child.as_object() {
        for (k, val) in c {
            match k.as_str() {
                "libraries" => {
                    let mut libs = val.as_array().cloned().unwrap_or_default();
                    libs.extend(parent["libraries"].as_array().cloned().unwrap_or_default());
                    o.insert(k.clone(), Value::Array(libs));
                }
                "arguments" => {
                    let mut args = parent["arguments"].as_object().cloned().unwrap_or_default();
                    for part in ["game", "jvm"] {
                        let mut a = parent["arguments"][part].as_array().cloned().unwrap_or_default();
                        a.extend(val[part].as_array().cloned().unwrap_or_default());
                        args.insert(part.to_string(), Value::Array(a));
                    }
                    o.insert(k.clone(), Value::Object(args));
                }
                "inheritsFrom" => {}
                _ => {
                    o.insert(k.clone(), val.clone());
                }
            }
        }
    }
    if !o.contains_key("_client_jar") {
        o.insert("_client_jar".into(), parent["id"].clone());
    }
    out
}

pub struct LibFile {
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

fn maven_parts(name: &str) -> Option<(String, String, String, Option<String>, String)> {
    let (n, ext) = match name.split_once('@') {
        Some((a, b)) => (a, b.to_string()),
        None => (name, "jar".to_string()),
    };
    let p: Vec<&str> = n.split(':').collect();
    if p.len() < 3 {
        return None;
    }
    Some((
        p[0].to_string(),
        p[1].to_string(),
        p[2].to_string(),
        p.get(3).map(|s| s.to_string()),
        ext,
    ))
}

pub fn maven_rel_path(name: &str) -> Option<String> {
    let (g, a, v, c, e) = maven_parts(name)?;
    let file = match c {
        Some(c) => format!("{a}-{v}-{c}.{e}"),
        None => format!("{a}-{v}.{e}"),
    };
    Some(format!("{}/{a}/{v}/{file}", g.replace('.', "/")))
}

/// Ключ для устранения дублей: group:artifact[:classifier]
pub fn lib_key(lib: &Value) -> String {
    let name = lib["name"].as_str().unwrap_or("");
    match maven_parts(name) {
        Some((g, a, _, c, _)) => format!("{g}:{a}:{}", c.unwrap_or_default()),
        None => name.to_string(),
    }
}

pub fn lib_is_natives_jar(lib: &Value) -> bool {
    lib["name"]
        .as_str()
        .and_then(maven_parts)
        .and_then(|p| p.3)
        .map(|c| c.starts_with("natives"))
        .unwrap_or(false)
}

pub fn lib_main_file(lib: &Value) -> Option<LibFile> {
    if let Some(a) = lib["downloads"].get("artifact").filter(|a| a.is_object()) {
        let path = a["path"]
            .as_str()
            .map(String::from)
            .or_else(|| maven_rel_path(lib["name"].as_str()?))?;
        return Some(LibFile {
            path,
            url: a["url"].as_str().unwrap_or("").to_string(),
            sha1: a["sha1"].as_str().map(String::from),
            size: a["size"].as_u64(),
        });
    }
    if lib["downloads"].is_object() {
        return None; // только natives-классификаторы
    }
    let name = lib["name"].as_str()?;
    let path = maven_rel_path(name)?;
    let mut base = lib["url"].as_str().unwrap_or("https://libraries.minecraft.net/").to_string();
    if !base.ends_with('/') {
        base.push('/');
    }
    Some(LibFile {
        url: format!("{base}{path}"),
        path,
        sha1: lib["sha1"].as_str().map(String::from),
        size: lib["size"].as_u64(),
    })
}

/// Старый стиль: natives + classifiers.
pub fn lib_native_file(lib: &Value) -> Option<LibFile> {
    let cls = lib["natives"][os_name()].as_str()?.replace("${arch}", "64");
    let c = lib["downloads"]["classifiers"].get(&cls)?;
    Some(LibFile {
        path: c["path"].as_str()?.to_string(),
        url: c["url"].as_str()?.to_string(),
        sha1: c["sha1"].as_str().map(String::from),
        size: c["size"].as_u64(),
    })
}

pub fn default_features() -> HashMap<String, bool> {
    let mut f = HashMap::new();
    f.insert("has_custom_resolution".to_string(), true);
    f.insert("has_quick_plays_support".to_string(), true);
    f
}

pub fn java_requirement(v: &Value) -> (String, u32) {
    (
        v["javaVersion"]["component"].as_str().unwrap_or("jre-legacy").to_string(),
        v["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32,
    )
}
