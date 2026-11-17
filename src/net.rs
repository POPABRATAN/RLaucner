use crate::mojang::Manifest;
use anyhow::{anyhow, bail, Result};
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default, Clone)]
pub struct Progress {
    pub text: String,
    pub done: usize,
    pub total: usize,
}
pub type Prog = Arc<Mutex<Progress>>;

pub fn set_prog(p: &Prog, text: impl Into<String>, done: usize, total: usize) {
    if let Ok(mut g) = p.lock() {
        g.text = text.into();
        g.done = done;
        g.total = total;
    }
}

/// Всё, что нужно фоновым задачам.
#[derive(Clone)]
pub struct Core {
    pub http: Client,
    pub root: PathBuf,
    pub manifest: Arc<Manifest>,
    pub prog: Prog,
    /// проверять хэши всех файлов заново («Обновить клиент»)
    pub force: bool,
}

pub fn http_client() -> Client {
    Client::builder()
        .user_agent(format!("RLauncher/{}", crate::config::APP_VERSION))
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(600))
        .build()
        .expect("http client")
}

pub async fn get_json<T: serde::de::DeserializeOwned>(c: &Client, url: &str) -> Result<T> {
    let r = c.get(url).send().await?;
    let s = r.status();
    if !s.is_success() {
        bail!("{url}: HTTP {s}");
    }
    Ok(r.json::<T>().await?)
}

pub async fn get_text(c: &Client, url: &str) -> Result<String> {
    let r = c.get(url).send().await?;
    let s = r.status();
    if !s.is_success() {
        bail!("{url}: HTTP {s}");
    }
    Ok(r.text().await?)
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(Sha1::digest(bytes))
}

pub fn file_sha1(p: &Path) -> Option<String> {
    std::fs::read(p).ok().map(|b| sha1_hex(&b))
}

/// Скачивает файл (с проверкой sha1 и размера, тремя попытками).
pub async fn download(
    c: &Client,
    url: &str,
    dest: &Path,
    sha1: Option<&str>,
    size: Option<u64>,
    verify: bool,
) -> Result<()> {
    if let Ok(md) = std::fs::metadata(dest) {
        let quick_ok = size == Some(md.len()) || (size.is_none() && sha1.is_none());
        if !verify && quick_ok {
            return Ok(());
        }
        if let Some(h) = sha1 {
            if file_sha1(dest).map(|x| x.eq_ignore_ascii_case(h)).unwrap_or(false) {
                return Ok(());
            }
        }
    }
    if let Some(p) = dest.parent() {
        tokio::fs::create_dir_all(p).await?;
    }
    let mut last = anyhow!("неизвестная ошибка");
    for _ in 0..3 {
        let res: Result<Vec<u8>> = async {
            let r = c.get(url).send().await?;
            let s = r.status();
            if !s.is_success() {
                bail!("{url}: HTTP {s}");
            }
            Ok(r.bytes().await?.to_vec())
        }
        .await;
        match res {
            Ok(bytes) => {
                if let Some(h) = sha1 {
                    if !sha1_hex(&bytes).eq_ignore_ascii_case(h) {
                        last = anyhow!("{url}: не совпала контрольная сумма");
                        continue;
                    }
                }
                let tmp = dest.with_extension("part");
                tokio::fs::write(&tmp, &bytes).await?;
                tokio::fs::rename(&tmp, dest).await?;
                return Ok(());
            }
            Err(e) => last = e,
        }
    }
    Err(last)
}

pub struct Task {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

/// Параллельная загрузка списка файлов с прогрессом.
pub async fn run_downloads(core: &Core, tasks: Vec<Task>, label: &str) -> Result<()> {
    use futures::StreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let total = tasks.len();
    set_prog(&core.prog, label, 0, total);
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicUsize::new(0));
    let errs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let label = label.to_string();
    futures::stream::iter(tasks)
        .for_each_concurrent(24, |t| {
            let http = core.http.clone();
            let prog = core.prog.clone();
            let done = done.clone();
            let errs = errs.clone();
            let label = label.clone();
            let verify = core.force;
            async move {
                if let Err(e) = download(&http, &t.url, &t.dest, t.sha1.as_deref(), t.size, verify).await {
                    if let Ok(mut g) = errs.lock() {
                        g.push(format!("{e:#}"));
                    }
                }
                let d = done.fetch_add(1, Ordering::SeqCst) + 1;
                set_prog(&prog, label, d, total);
            }
        })
        .await;
    let e = errs.lock().map(|g| g.clone()).unwrap_or_default();
    if let Some(first) = e.first() {
        bail!("Не удалось скачать {} файл(ов). Первая ошибка: {first}", e.len());
    }
    Ok(())
}
