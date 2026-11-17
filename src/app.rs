use crate::config::*;
use crate::modrinth::{self, Hit, SearchParams};
use crate::mojang::*;
use crate::net::*;
use crate::{auth, install, java, launch, loaders};
use eframe::egui;
use std::collections::HashSet;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Splash,
    Main,
    Catalog,
    Settings,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CatTab {
    Modpack,
    Mod,
    ResourcePack,
    Shader,
    DataPack,
}
impl CatTab {
    pub const ALL: [CatTab; 5] = [CatTab::Modpack, CatTab::Mod, CatTab::ResourcePack, CatTab::Shader, CatTab::DataPack];
    pub fn ptype(&self) -> &'static str {
        match self {
            CatTab::Modpack => "modpack",
            CatTab::Mod => "mod",
            CatTab::ResourcePack => "resourcepack",
            CatTab::Shader => "shader",
            CatTab::DataPack => "datapack",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            CatTab::Modpack => "Мод-паки",
            CatTab::Mod => "Моды",
            CatTab::ResourcePack => "Ресурс-паки",
            CatTab::Shader => "Шейдеры",
            CatTab::DataPack => "Датапаки",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Sort {
    Downloads,
    Relevance,
    Follows,
    Newest,
    Updated,
}
impl Sort {
    pub const ALL: [Sort; 5] = [Sort::Downloads, Sort::Relevance, Sort::Follows, Sort::Newest, Sort::Updated];
    pub fn index(&self) -> &'static str {
        match self {
            Sort::Downloads => "downloads",
            Sort::Relevance => "relevance",
            Sort::Follows => "follows",
            Sort::Newest => "newest",
            Sort::Updated => "updated",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Sort::Downloads => "Популярные",
            Sort::Relevance => "По релевантности",
            Sort::Follows => "По подпискам",
            Sort::Newest => "Новые",
            Sort::Updated => "Обновлённые",
        }
    }
}

pub enum Msg {
    Manifest(Result<Manifest, String>),
    Search { gen: u64, append: bool, res: Result<modrinth::SearchResult, String> },
    LoaderVersions { loader: Loader, mc: String, res: Result<Vec<String>, String> },
    ProfileReady(Result<Profile, String>),
    ModInstalled { id: String, res: Result<Vec<String>, String> },
    Device(Result<auth::DeviceCode, String>),
    MsDone(Result<Account, String>),
    AccountUpdated(Account),
    Launched,
    GameExited(String),
    Toast(String),
    BusyDone,
}

pub struct CreateForm {
    pub name: String,
    pub loader: Loader,
    pub mc: String,
    pub lvs: Vec<String>,
    pub lv: String,
    pub fetched: Option<(Loader, String)>,
    pub fetching: bool,
    pub fetch_err: Option<String>,
    pub newest_first: bool,
    pub filter: String,
}

pub struct Catalog {
    pub tab: CatTab,
    pub query: String,
    pub sort: Sort,
    pub game_version: String,
    pub hits: Vec<Hit>,
    pub total: u64,
    pub loading: bool,
    pub gen: u64,
    pub error: Option<String>,
    pub filters_open: bool,
    pub use_loader: bool,
    pub last_key: Option<(CatTab, String, Sort, bool, Option<usize>)>,
    pub installing: HashSet<String>,
    pub done: HashSet<String>,
}

pub struct App {
    pub rt: Arc<tokio::runtime::Runtime>,
    pub http: reqwest::Client,
    pub cfg: Config,
    pub screen: Screen,
    pub started: Instant,
    pub tx: Sender<Msg>,
    pub rx: Receiver<Msg>,
    pub manifest: Option<Arc<Manifest>>,
    pub manifest_done: bool,
    pub manifest_err: Option<String>,
    pub prog: Prog,
    pub busy: bool,
    pub busy_label: String,
    pub toast: Option<(String, Instant)>,
    pub show_versions: bool,
    pub show_accounts: bool,
    pub show_add_server: bool,
    pub show_help: bool,
    pub cv: CreateForm,
    pub acc_name: String,
    pub ms_device: Option<auth::DeviceCode>,
    pub ms_running: bool,
    pub srv_name: String,
    pub srv_addr: String,
    pub settings_tab: usize,
    pub cat: Catalog,
    pub force_update: bool,
    pub first_run_checked: bool,
    pub game_running: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> App {
        egui_extras::install_image_loaders(&cc.egui_ctx);
        crate::ui::setup_style(&cc.egui_ctx);
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(4)
                .enable_all()
                .build()
                .expect("tokio"),
        );
        let (tx, rx) = channel();
        let http = http_client();
        let cfg = Config::load();
        // загрузка списка версий
        {
            let (http, tx, ctx) = (http.clone(), tx.clone(), cc.egui_ctx.clone());
            rt.spawn(async move {
                let r = fetch_manifest(&http).await.map_err(|e| format!("{e:#}"));
                let _ = tx.send(Msg::Manifest(r));
                ctx.request_repaint();
            });
        }
        let cat_gv = String::new();
        App {
            rt,
            http,
            screen: Screen::Splash,
            started: Instant::now(),
            tx,
            rx,
            manifest: None,
            manifest_done: false,
            manifest_err: None,
            prog: Arc::new(Mutex::new(Progress::default())),
            busy: false,
            busy_label: String::new(),
            toast: None,
            show_versions: false,
            show_accounts: false,
            show_add_server: false,
            show_help: false,
            cv: CreateForm {
                name: String::new(),
                loader: Loader::Fabric,
                mc: String::new(),
                lvs: vec![],
                lv: String::new(),
                fetched: None,
                fetching: false,
                fetch_err: None,
                newest_first: true,
                filter: String::new(),
            },
            acc_name: String::new(),
            ms_device: None,
            ms_running: false,
            srv_name: String::new(),
            srv_addr: String::new(),
            settings_tab: 0,
            cat: Catalog {
                tab: CatTab::Mod,
                query: String::new(),
                sort: Sort::Downloads,
                game_version: cat_gv,
                hits: vec![],
                total: 0,
                loading: false,
                gen: 0,
                error: None,
                filters_open: false,
                use_loader: true,
                last_key: None,
                installing: HashSet::new(),
                done: HashSet::new(),
            },
            force_update: false,
            first_run_checked: false,
            game_running: false,
            cfg,
        }
    }

    pub fn toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    pub fn core(&self, force: bool) -> Core {
        Core {
            http: self.http.clone(),
            root: self.cfg.root_dir.clone(),
            manifest: self.manifest.clone().unwrap_or_else(|| Arc::new(Manifest::default())),
            prog: self.prog.clone(),
            force,
        }
    }

    pub fn begin_busy(&mut self, label: &str) {
        self.busy = true;
        self.busy_label = label.to_string();
        set_prog(&self.prog, label, 0, 0);
    }

    /// Версии игры из списка Mojang с учётом галочек из настроек.
    pub fn visible_versions(&self) -> Vec<ManifestVersion> {
        let Some(m) = &self.manifest else { return vec![] };
        let mut v: Vec<ManifestVersion> = m
            .versions
            .iter()
            .filter(|x| match x.kind.as_str() {
                "release" => self.cfg.show_old || x.release_time.as_str() >= "2013-04-25",
                "snapshot" => self.cfg.show_snapshots,
                "old_beta" => self.cfg.show_beta,
                "old_alpha" => self.cfg.show_alpha,
                _ => true,
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| b.release_time.cmp(&a.release_time));
        if !self.cv.newest_first {
            v.reverse();
        }
        v
    }

    pub fn pump(&mut self, ctx: &egui::Context) {
        while let Ok(m) = self.rx.try_recv() {
            match m {
                Msg::Manifest(r) => {
                    self.manifest_done = true;
                    match r {
                        Ok(m) => {
                            if self.cv.mc.is_empty() {
                                self.cv.mc = m.latest.release.clone();
                            }
                            self.manifest = Some(Arc::new(m));
                        }
                        Err(e) => self.manifest_err = Some(e),
                    }
                }
                Msg::Search { gen, append, res } => {
                    if gen != self.cat.gen {
                        continue;
                    }
                    self.cat.loading = false;
                    match res {
                        Ok(r) => {
                            self.cat.total = r.total_hits;
                            if append {
                                self.cat.hits.extend(r.hits);
                            } else {
                                self.cat.hits = r.hits;
                            }
                        }
                        Err(e) => self.cat.error = Some(e),
                    }
                }
                Msg::LoaderVersions { loader, mc, res } => {
                    if self.cv.loader == loader && self.cv.mc == mc {
                        self.cv.fetching = false;
                        self.cv.fetched = Some((loader, mc));
                        match res {
                            Ok(v) => {
                                self.cv.fetch_err = None;
                                self.cv.lv = v.first().cloned().unwrap_or_default();
                                self.cv.lvs = v;
                            }
                            Err(e) => {
                                self.cv.lvs.clear();
                                self.cv.lv.clear();
                                self.cv.fetch_err = Some(e);
                            }
                        }
                    }
                }
                Msg::ProfileReady(r) => match r {
                    Ok(p) => {
                        let name = p.name.clone();
                        self.cfg.profiles.push(p);
                        self.cfg.selected_profile = Some(self.cfg.profiles.len() - 1);
                        self.cfg.save();
                        self.toast(format!("Версия «{name}» готова"));
                        self.cv.name.clear();
                    }
                    Err(e) => self.toast(format!("Ошибка: {e}")),
                },
                Msg::ModInstalled { id, res } => {
                    self.cat.installing.remove(&id);
                    match res {
                        Ok(files) => {
                            self.cat.done.insert(id);
                            self.toast(format!("Установлено: {}", files.join(", ")));
                        }
                        Err(e) => self.toast(format!("Не удалось установить: {e}")),
                    }
                }
                Msg::Device(r) => match r {
                    Ok(dc) => {
                        let _ = open::that(&dc.verification_uri);
                        self.ms_device = Some(dc.clone());
                        let (http, tx, ctx2, cid) = (self.http.clone(), self.tx.clone(), ctx.clone(), self.cfg.ms_client_id.clone());
                        self.rt.spawn(async move {
                            let r = auth::poll_login(&http, &cid, &dc).await.map_err(|e| format!("{e:#}"));
                            let _ = tx.send(Msg::MsDone(r));
                            ctx2.request_repaint();
                        });
                    }
                    Err(e) => {
                        self.ms_running = false;
                        self.toast(format!("Вход Microsoft: {e}"));
                    }
                },
                Msg::MsDone(r) => {
                    self.ms_running = false;
                    self.ms_device = None;
                    match r {
                        Ok(a) => {
                            let n = a.name.clone();
                            self.upsert_account(a, true);
                            self.toast(format!("Вы вошли как {n}"));
                        }
                        Err(e) => self.toast(format!("Вход Microsoft: {e}")),
                    }
                }
                Msg::AccountUpdated(a) => self.upsert_account(a, false),
                Msg::Launched => {
                    self.game_running = true;
                    self.toast("Игра запускается…");
                    if self.cfg.hide_on_launch {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                }
                Msg::GameExited(s) => {
                    self.game_running = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    self.toast(s);
                }
                Msg::Toast(s) => self.toast(s),
                Msg::BusyDone => self.busy = false,
            }
        }
    }

    fn upsert_account(&mut self, a: Account, select: bool) {
        if let Some(i) = self.cfg.accounts.iter().position(|x| x.uuid == a.uuid && x.kind == a.kind) {
            self.cfg.accounts[i] = a;
            if select {
                self.cfg.selected_account = i;
            }
        } else {
            self.cfg.accounts.push(a);
            if select {
                self.cfg.selected_account = self.cfg.accounts.len() - 1;
            }
        }
        self.cfg.save();
    }

    pub fn add_offline(&mut self, name: &str) {
        if !auth::valid_nick(name) {
            self.toast("Ник: 3–16 символов, только латиница, цифры и _");
            return;
        }
        self.upsert_account(auth::offline_account(name), true);
        self.toast(format!("Офлайн-аккаунт {name} добавлен"));
    }

    pub fn start_ms_login(&mut self, ctx: &egui::Context) {
        if self.ms_running {
            return;
        }
        self.ms_running = true;
        let (http, tx, ctx2, cid) = (self.http.clone(), self.tx.clone(), ctx.clone(), self.cfg.ms_client_id.clone());
        self.rt.spawn(async move {
            let r = auth::device_code(&http, &cid).await.map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Device(r));
            ctx2.request_repaint();
        });
    }

    pub fn fetch_loader_versions(&mut self, ctx: &egui::Context) {
        let (loader, mc) = (self.cv.loader, self.cv.mc.clone());
        self.cv.fetching = true;
        self.cv.fetch_err = None;
        let (http, tx, ctx2) = (self.http.clone(), self.tx.clone(), ctx.clone());
        self.rt.spawn(async move {
            let res = loaders::loader_versions(&http, loader, &mc).await.map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::LoaderVersions { loader, mc, res });
            ctx2.request_repaint();
        });
    }

    pub fn create_version(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let name = if self.cv.name.trim().is_empty() {
            format!("{} {}", self.cv.loader.short(), self.cv.mc)
        } else {
            self.cv.name.trim().to_string()
        };
        if self.cfg.profiles.iter().any(|p| p.name == name) {
            self.toast("Версия с таким названием уже есть");
            return;
        }
        self.begin_busy(&format!("Создание версии «{name}»"));
        let core = self.core(false);
        let (loader, mc, lv) = (self.cv.loader, self.cv.mc.clone(), self.cv.lv.clone());
        let dir = self.cfg.instances_dir().join(safe_name(&name));
        let (tx, ctx2) = (self.tx.clone(), ctx.clone());
        self.rt.spawn(async move {
            let r: anyhow::Result<Profile> = async {
                install::install_version(&core, &mc).await?;
                let id = loaders::install_loader(&core, loader, &mc, &lv).await?;
                install::install_version(&core, &id).await?;
                Ok(Profile { name, mc_version: mc, loader, loader_version: lv, version_id: id, game_dir: dir })
            }
            .await;
            let _ = tx.send(Msg::ProfileReady(r.map_err(|e| format!("{e:#}"))));
            let _ = tx.send(Msg::BusyDone);
            ctx2.request_repaint();
        });
    }

    pub fn start_search(&mut self, ctx: &egui::Context, append: bool) {
        if append && self.cat.loading {
            return;
        }
        if !append {
            self.cat.gen += 1;
            self.cat.hits.clear();
            self.cat.total = 0;
        }
        self.cat.loading = true;
        self.cat.error = None;
        let gen = self.cat.gen;
        let loaders = if self.cat.use_loader {
            self.cfg.profile().map(|p| p.loader.modrinth_loaders()).unwrap_or_default()
        } else {
            vec![]
        };
        let params = SearchParams {
            project_type: self.cat.tab.ptype(),
            query: self.cat.query.clone(),
            game_version: self.cat.game_version.clone(),
            loaders,
            index: self.cat.sort.index(),
            offset: if append { self.cat.hits.len() } else { 0 },
        };
        let (http, tx, ctx2) = (self.http.clone(), self.tx.clone(), ctx.clone());
        self.rt.spawn(async move {
            let res = modrinth::search(&http, &params).await.map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::Search { gen, append, res });
            ctx2.request_repaint();
        });
    }

    pub fn install_hit(&mut self, ctx: &egui::Context, hit: &Hit) {
        let tab = self.cat.tab;
        let (tx, ctx2) = (self.tx.clone(), ctx.clone());
        if tab == CatTab::Modpack {
            if self.busy {
                self.toast("Дождитесь окончания текущей операции");
                return;
            }
            self.begin_busy(&format!("Установка сборки «{}»", hit.title));
            let core = self.core(false);
            let (id, title, inst) = (hit.project_id.clone(), hit.title.clone(), self.cfg.instances_dir());
            self.rt.spawn(async move {
                let r: anyhow::Result<Profile> = async {
                    let info = modrinth::install_modpack(&core, &id, &title, &inst).await?;
                    install::install_version(&core, &info.mc).await?;
                    let vid = loaders::install_loader(&core, info.loader, &info.mc, &info.loader_version).await?;
                    install::install_version(&core, &vid).await?;
                    Ok(Profile { name: info.name, mc_version: info.mc, loader: info.loader, loader_version: info.loader_version, version_id: vid, game_dir: info.game_dir })
                }
                .await;
                let _ = tx.send(Msg::ProfileReady(r.map_err(|e| format!("{e:#}"))));
                let _ = tx.send(Msg::BusyDone);
                ctx2.request_repaint();
            });
            return;
        }
        let Some(profile) = self.cfg.profile().cloned() else {
            self.toast("Сначала выберите или создайте версию (кнопка «Версии» внизу)");
            self.show_versions = true;
            return;
        };
        self.cat.installing.insert(hit.project_id.clone());
        let core = self.core(false);
        let (id, gv) = (hit.project_id.clone(), self.cat.game_version.clone());
        self.rt.spawn(async move {
            let res = modrinth::install_project(&core, &id, tab.ptype(), &profile, &gv).await.map_err(|e| format!("{e:#}"));
            let _ = tx.send(Msg::ModInstalled { id, res });
            ctx2.request_repaint();
        });
    }

    pub fn start_launch(&mut self, ctx: &egui::Context, server: Option<String>) {
        if self.busy {
            return;
        }
        let Some(acc) = self.cfg.account().cloned() else {
            self.toast("Сначала добавьте аккаунт");
            self.show_accounts = true;
            return;
        };
        let Some(profile) = self.cfg.profile().cloned() else {
            self.toast("Сначала создайте версию");
            self.show_versions = true;
            return;
        };
        self.begin_busy(&format!("Подготовка «{}»", profile.name));
        let core = self.core(self.force_update);
        self.force_update = false;
        let cfg = self.cfg.clone();
        let (tx, ctx2) = (self.tx.clone(), ctx.clone());
        self.rt.spawn(async move {
            if let Err(e) = launch_task(core, cfg, profile, acc, server, tx.clone()).await {
                let _ = tx.send(Msg::Toast(format!("Ошибка запуска: {e:#}")));
            }
            let _ = tx.send(Msg::BusyDone);
            ctx2.request_repaint();
        });
    }
}

async fn launch_task(core: Core, cfg: Config, profile: Profile, mut acc: Account, server: Option<String>, tx: Sender<Msg>) -> anyhow::Result<()> {
    install::install_version(&core, &profile.version_id).await?;
    if acc.kind == AccountKind::Microsoft && now_secs() >= acc.expires_at {
        set_prog(&core.prog, "Обновление входа Microsoft", 0, 0);
        acc = auth::refresh(&core.http, &cfg.ms_client_id, &acc).await?;
        let _ = tx.send(Msg::AccountUpdated(acc.clone()));
    }
    let v = resolve(&core.root, &profile.version_id)?;
    let (component, _) = java_requirement(&v);
    let jbin = java::ensure_java(&core, &component, &cfg.java_path, false).await?;
    std::fs::create_dir_all(&profile.game_dir)?;
    if cfg.ru_lang {
        let opt = profile.game_dir.join("options.txt");
        if !opt.exists() {
            let _ = std::fs::write(&opt, "lang:ru_ru\n");
        }
    }
    let mut cmd = launch::build_command(&core.root, &profile, &acc, &cfg, &jbin, server.as_deref())?;
    let lp = launch::log_path(&core.root);
    std::fs::create_dir_all(lp.parent().unwrap())?;
    let f = std::fs::File::create(&lp)?;
    cmd.stdout(f.try_clone()?).stderr(f);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let mut child = cmd.spawn().map_err(|e| anyhow::anyhow!("не удалось запустить Java: {e}"))?;
    let _ = tx.send(Msg::Launched);
    let tx2 = tx.clone();
    tokio::task::spawn_blocking(move || {
        let status = child.wait();
        let msg = match status {
            Ok(s) if s.success() => "Игра закрыта".to_string(),
            Ok(s) => {
                let log = std::fs::read_to_string(&lp).unwrap_or_default();
                let tail: Vec<&str> = log.lines().rev().take(5).collect();
                format!(
                    "Игра завершилась с кодом {}. Последние строки лога ({}):\n{}",
                    s.code().unwrap_or(-1),
                    lp.display(),
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                )
            }
            Err(e) => format!("Ошибка ожидания игры: {e}"),
        };
        let _ = tx2.send(Msg::GameExited(msg));
    });
    Ok(())
}
