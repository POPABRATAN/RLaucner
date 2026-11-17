use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl Loader {
    pub const ALL: [Loader; 5] = [
        Loader::Vanilla,
        Loader::Forge,
        Loader::Fabric,
        Loader::Quilt,
        Loader::NeoForge,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla (без модов)",
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
        }
    }
    pub fn short(&self) -> &'static str {
        match self {
            Loader::Vanilla => "Vanilla",
            Loader::Fabric => "Fabric",
            Loader::Quilt => "Quilt",
            Loader::Forge => "Forge",
            Loader::NeoForge => "NeoForge",
        }
    }
    /// Какие загрузчики Modrinth подходят для этого профиля.
    pub fn modrinth_loaders(&self) -> Vec<&'static str> {
        match self {
            Loader::Vanilla => vec![],
            Loader::Fabric => vec!["fabric"],
            Loader::Quilt => vec!["quilt", "fabric"],
            Loader::Forge => vec!["forge"],
            Loader::NeoForge => vec!["neoforge"],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub name: String,
    pub mc_version: String,
    pub loader: Loader,
    pub loader_version: String,
    /// id версии, с которой запускается игра (versions/<id>/<id>.json)
    pub version_id: String,
    pub game_dir: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum AccountKind {
    Offline,
    Microsoft,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Account {
    pub kind: AccountKind,
    pub name: String,
    pub uuid: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_at: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServerEntry {
    pub name: String,
    pub addr: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub root_dir: PathBuf,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub show_snapshots: bool,
    pub show_modded: bool,
    pub show_old: bool,
    pub show_beta: bool,
    pub show_alpha: bool,
    pub game_args: String,
    pub java_path: String,
    pub memory_mb: u32,
    pub ms_client_id: String,
    pub accounts: Vec<Account>,
    pub selected_account: usize,
    pub profiles: Vec<Profile>,
    pub selected_profile: Option<usize>,
    pub servers: Vec<ServerEntry>,
    pub hide_on_launch: bool,
    pub ru_lang: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            root_dir: dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".minecraft"),
            width: 925,
            height: 530,
            fullscreen: false,
            show_snapshots: false,
            show_modded: true,
            show_old: true,
            show_beta: false,
            show_alpha: false,
            game_args: String::new(),
            java_path: String::new(),
            memory_mb: 2048,
            ms_client_id: std::env::var("RLAUNCHER_MS_CLIENT_ID").unwrap_or_default(),
            accounts: vec![],
            selected_account: 0,
            profiles: vec![],
            selected_profile: None,
            servers: vec![],
            hide_on_launch: false,
            ru_lang: true,
        }
    }
}

pub fn launcher_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".rlauncher")
}

impl Config {
    pub fn path() -> PathBuf {
        launcher_dir().join("config.json")
    }
    pub fn load() -> Config {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self) {
        let p = Self::path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
    pub fn instances_dir(&self) -> PathBuf {
        self.root_dir.join("instances")
    }
    pub fn profile(&self) -> Option<&Profile> {
        self.selected_profile.and_then(|i| self.profiles.get(i))
    }
    pub fn account(&self) -> Option<&Account> {
        self.accounts.get(self.selected_account)
    }
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Имя папки без запрещённых символов.
pub fn safe_name(s: &str) -> String {
    let t: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' })
        .collect();
    let t = t.trim().trim_matches('.').to_string();
    if t.is_empty() { "profile".into() } else { t }
}
