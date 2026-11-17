use crate::config::{safe_name, Loader, Profile};
use crate::net::*;
use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const API: &str = "https://api.modrinth.com/v2";

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Hit {
    pub project_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub project_type: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total_hits: u64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct MFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub hashes: Hashes,
    #[serde(default)]
    pub size: u64,
}
#[derive(Deserialize, Clone, Debug, Default)]
pub struct Hashes {
    #[serde(default)]
    pub sha1: String,
}
#[derive(Deserialize, Clone, Debug)]
pub struct Dep {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}
#[derive(Deserialize, Clone, Debug)]
pub struct MVersion {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    pub files: Vec<MFile>,
    #[serde(default)]
    pub dependencies: Vec<Dep>,
}

pub struct SearchParams {
    pub project_type: &'static str,
    pub query: String,
    pub game_version: String, // пусто = любая
    pub loaders: Vec<&'static str>,
    pub index: &'static str,
    pub offset: usize,
}

pub async fn search(c: &reqwest::Client, p: &SearchParams) -> Result<SearchResult> {
    let mut facets: Vec<Value> = vec![json!([format!("project_type:{}", p.project_type)])];
    if !p.game_version.is_empty() {
        facets.push(json!([format!("versions:{}", p.game_version)]));
    }
    if !p.loaders.is_empty() && matches!(p.project_type, "mod" | "modpack") {
        facets.push(Value::Array(p.loaders.iter().map(|l| json!(format!("categories:{l}"))).collect::<Vec<_>>()));
    }
    let r = c
        .get(format!("{API}/search"))
        .query(&[
            ("query", p.query.as_str()),
            ("facets", &Value::Array(facets).to_string()),
            ("index", p.index),
            ("limit", "20"),
            ("offset", &p.offset.to_string()),
        ])
        .send()
        .await?;
    if !r.status().is_success() {
        bail!("Modrinth: HTTP {}", r.status());
    }
    Ok(r.json().await?)
}

fn folder_for(ptype: &str) -> &'static str {
    match ptype {
        "mod" => "mods",
        "resourcepack" => "resourcepacks",
        "shader" => "shaderpacks",
        "datapack" => "datapacks",
        _ => "mods",
    }
}

async fn versions_of(c: &reqwest::Client, project: &str, mc: &str, loaders: &[&str]) -> Result<Vec<MVersion>> {
    let mut q: Vec<(&str, String)> = vec![];
    if !mc.is_empty() {
        q.push(("game_versions", json!([mc]).to_string()));
    }
    if !loaders.is_empty() {
        q.push(("loaders", json!(loaders).to_string()));
    }
    let r = c.get(format!("{API}/project/{project}/version")).query(&q).send().await?;
    if !r.status().is_success() {
        bail!("Modrinth: HTTP {}", r.status());
    }
    Ok(r.json().await?)
}

async fn version_by_id(c: &reqwest::Client, id: &str) -> Result<MVersion> {
    let r = c.get(format!("{API}/version/{id}")).send().await?;
    if !r.status().is_success() {
        bail!("Modrinth: HTTP {}", r.status());
    }
    Ok(r.json().await?)
}

fn primary(v: &MVersion) -> Result<&MFile> {
    v.files
        .iter()
        .find(|f| f.primary)
        .or_else(|| v.files.first())
        .ok_or_else(|| anyhow!("у версии нет файлов"))
}

/// Скачивает проект именно под версию игры и загрузчик выбранного профиля
/// (для модов — вместе с обязательными зависимостями).
pub async fn install_project(core: &Core, project_id: &str, ptype: &str, profile: &Profile, game_version: &str) -> Result<Vec<String>> {
    let dir = profile.game_dir.join(folder_for(ptype));
    std::fs::create_dir_all(&dir)?;
    let loaders: Vec<&str> = if ptype == "mod" { profile.loader.modrinth_loaders() } else { vec![] };
    if ptype == "mod" && profile.loader == Loader::Vanilla {
        bail!("У выбранной версии нет загрузчика модов. Создайте версию с Fabric, Forge, Quilt или NeoForge.");
    }
    let mc = if game_version.is_empty() { profile.mc_version.as_str() } else { game_version };
    let mut queue: Vec<(String, Option<String>)> = vec![(project_id.to_string(), None)];
    let mut seen = HashSet::new();
    let mut installed = vec![];
    while let Some((pid, vid)) = queue.pop() {
        if !seen.insert(pid.clone()) {
            continue;
        }
        let first = installed.is_empty();
        let ver = match vid {
            Some(v) => version_by_id(&core.http, &v).await?,
            None => versions_of(&core.http, &pid, mc, &loaders)
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| {
                    anyhow!(
                        "Нет совместимой версии для Minecraft {mc}{}",
                        if loaders.is_empty() { String::new() } else { format!(" ({})", profile.loader.short()) }
                    )
                })?,
        };
        let f = primary(&ver)?;
        set_prog(&core.prog, format!("Скачивание {}", f.filename), 0, 1);
        let name = Path::new(&f.filename).file_name().ok_or_else(|| anyhow!("плохое имя файла"))?.to_owned();
        let sha = if f.hashes.sha1.is_empty() { None } else { Some(f.hashes.sha1.as_str()) };
        download(&core.http, &f.url, &dir.join(&name), sha, if f.size > 0 { Some(f.size) } else { None }, false).await?;
        installed.push(f.filename.clone());
        if ptype == "mod" {
            for d in &ver.dependencies {
                if d.dependency_type == "required" {
                    if let Some(p) = &d.project_id {
                        queue.push((p.clone(), d.version_id.clone()));
                    }
                }
            }
        }
        let _ = first;
    }
    Ok(installed)
}

fn safe_rel(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    if path.is_absolute() {
        return None;
    }
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if out.as_os_str().is_empty() { None } else { Some(out) }
}

pub struct PackInfo {
    pub name: String,
    pub mc: String,
    pub loader: Loader,
    pub loader_version: String,
    pub game_dir: PathBuf,
}

/// Скачивает .mrpack, раскладывает файлы по папке нового профиля.
pub async fn install_modpack(core: &Core, project_id: &str, title: &str, instances_dir: &Path) -> Result<PackInfo> {
    let versions = versions_of(&core.http, project_id, "", &[]).await?;
    let ver = versions.into_iter().next().ok_or_else(|| anyhow!("у сборки нет версий"))?;
    let f = ver
        .files
        .iter()
        .find(|f| f.filename.ends_with(".mrpack"))
        .ok_or_else(|| anyhow!("в сборке нет файла .mrpack"))?;
    let tmp = core.root.join("tmp").join(&f.filename);
    set_prog(&core.prog, format!("Скачивание сборки {title}"), 0, 1);
    download(&core.http, &f.url, &tmp, if f.hashes.sha1.is_empty() { None } else { Some(f.hashes.sha1.as_str()) }, None, false).await?;

    let data = std::fs::read(&tmp)?;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(data))?;
    let index: Value = {
        let mut e = z.by_name("modrinth.index.json").map_err(|_| anyhow!("в архиве нет modrinth.index.json"))?;
        let mut s = String::new();
        e.read_to_string(&mut s)?;
        serde_json::from_str(&s)?
    };
    let deps = &index["dependencies"];
    let mc = deps["minecraft"].as_str().ok_or_else(|| anyhow!("в сборке не указана версия Minecraft"))?.to_string();
    let (loader, lv) = if let Some(v) = deps["fabric-loader"].as_str() {
        (Loader::Fabric, v.to_string())
    } else if let Some(v) = deps["quilt-loader"].as_str() {
        (Loader::Quilt, v.to_string())
    } else if let Some(v) = deps["neoforge"].as_str() {
        (Loader::NeoForge, v.to_string())
    } else if let Some(v) = deps["forge"].as_str() {
        (Loader::Forge, v.to_string())
    } else {
        (Loader::Vanilla, String::new())
    };
    let pack_name = index["name"].as_str().unwrap_or(title).to_string();
    let mut dir = instances_dir.join(safe_name(&pack_name));
    let mut n = 2;
    while dir.exists() {
        dir = instances_dir.join(format!("{} ({n})", safe_name(&pack_name)));
        n += 1;
    }
    std::fs::create_dir_all(&dir)?;

    let mut tasks = vec![];
    for file in index["files"].as_array().cloned().unwrap_or_default() {
        if file["env"]["client"].as_str() == Some("unsupported") {
            continue;
        }
        let Some(rel) = file["path"].as_str().and_then(safe_rel) else { continue };
        let Some(url) = file["downloads"][0].as_str() else { continue };
        tasks.push(Task {
            url: url.to_string(),
            dest: dir.join(rel),
            sha1: file["hashes"]["sha1"].as_str().map(String::from),
            size: file["fileSize"].as_u64(),
        });
    }
    run_downloads(core, tasks, "Скачивание файлов сборки").await?;

    // overrides
    for i in 0..z.len() {
        let mut e = z.by_index(i)?;
        if e.is_dir() {
            continue;
        }
        let name = e.name().to_string();
        let rel = name
            .strip_prefix("overrides/")
            .or_else(|| name.strip_prefix("client-overrides/"));
        if let Some(rel) = rel.and_then(safe_rel) {
            let dest = dir.join(rel);
            if let Some(p) = dest.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut buf = vec![];
            e.read_to_end(&mut buf)?;
            std::fs::write(dest, buf)?;
        }
    }
    let _ = std::fs::remove_file(&tmp);
    Ok(PackInfo { name: pack_name, mc, loader, loader_version: lv, game_dir: dir })
}
