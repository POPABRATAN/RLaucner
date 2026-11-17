use crate::config::{now_secs, Account, AccountKind};
use anyhow::{anyhow, bail, Result};
use md5::{Digest, Md5};
use reqwest::Client;
use serde_json::{json, Value};

/// Офлайн-UUID так же, как его считает сам Minecraft/серверы (UUID v3 от "OfflinePlayer:<ник>").
pub fn offline_uuid(name: &str) -> String {
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    hex::encode(h)
}

pub fn offline_account(name: &str) -> Account {
    Account {
        kind: AccountKind::Offline,
        name: name.to_string(),
        uuid: offline_uuid(name),
        access_token: String::new(),
        refresh_token: String::new(),
        expires_at: 0,
    }
}

pub fn valid_nick(n: &str) -> bool {
    (3..=16).contains(&n.len()) && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Clone, Debug)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

pub async fn device_code(c: &Client, client_id: &str) -> Result<DeviceCode> {
    if client_id.trim().is_empty() {
        bail!("Не указан Azure Client ID (Настройки → Настройки RLauncher).");
    }
    let r: Value = c
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", client_id.trim()), ("scope", SCOPE)])
        .send()
        .await?
        .json()
        .await?;
    if let Some(e) = r["error_description"].as_str() {
        bail!("{e}");
    }
    Ok(DeviceCode {
        user_code: r["user_code"].as_str().ok_or_else(|| anyhow!("нет user_code"))?.into(),
        device_code: r["device_code"].as_str().ok_or_else(|| anyhow!("нет device_code"))?.into(),
        verification_uri: r["verification_uri"].as_str().unwrap_or("https://microsoft.com/link").into(),
        interval: r["interval"].as_u64().unwrap_or(5),
        expires_in: r["expires_in"].as_u64().unwrap_or(900),
    })
}

pub async fn poll_login(c: &Client, client_id: &str, dc: &DeviceCode) -> Result<Account> {
    let mut interval = dc.interval.max(2);
    let deadline = now_secs() + dc.expires_in;
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        if now_secs() > deadline {
            bail!("Время ожидания входа истекло.");
        }
        let r: Value = c
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id.trim()),
                ("device_code", dc.device_code.as_str()),
            ])
            .send()
            .await?
            .json()
            .await?;
        match r["error"].as_str() {
            Some("authorization_pending") => continue,
            Some("slow_down") => {
                interval += 5;
                continue;
            }
            Some(_) => bail!("{}", r["error_description"].as_str().unwrap_or("вход отклонён")),
            None => {
                let at = r["access_token"].as_str().ok_or_else(|| anyhow!("нет access_token"))?;
                let rt = r["refresh_token"].as_str().unwrap_or("");
                return finish_login(c, at, rt).await;
            }
        }
    }
}

pub async fn refresh(c: &Client, client_id: &str, acc: &Account) -> Result<Account> {
    let r: Value = c
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.trim()),
            ("refresh_token", acc.refresh_token.as_str()),
            ("scope", SCOPE),
        ])
        .send()
        .await?
        .json()
        .await?;
    if let Some(e) = r["error_description"].as_str() {
        bail!("Нужно войти заново: {e}");
    }
    let at = r["access_token"].as_str().ok_or_else(|| anyhow!("нет access_token"))?;
    let rt = r["refresh_token"].as_str().unwrap_or(&acc.refresh_token);
    finish_login(c, at, rt).await
}

async fn finish_login(c: &Client, ms_token: &str, refresh: &str) -> Result<Account> {
    // 1. Xbox Live
    let xbl: Value = c
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_token}")},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json()
        .await?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| anyhow!("Xbox Live: нет токена"))?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| anyhow!("Xbox Live: нет uhs"))?;
    // 2. XSTS
    let r = c
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;
    let xsts: Value = r.json().await?;
    if let Some(code) = xsts["XErr"].as_u64() {
        bail!(match code {
            2148916233 => "У этой учётной записи Microsoft нет профиля Xbox. Создайте его на xbox.com.".to_string(),
            2148916235 => "Xbox Live недоступен в вашей стране.".to_string(),
            2148916238 => "Для детской учётной записи нужно разрешение родителя (семейная группа Microsoft).".to_string(),
            c => format!("Ошибка Xbox: {c}"),
        });
    }
    let xsts_token = xsts["Token"].as_str().ok_or_else(|| anyhow!("XSTS: нет токена"))?;
    // 3. Minecraft
    let mc: Value = c
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken": format!("XBL3.0 x={uhs};{xsts_token}")}))
        .send()
        .await?
        .json()
        .await?;
    let mc_token = mc["access_token"].as_str().ok_or_else(|| anyhow!("Minecraft: вход не удался ({})", mc["errorMessage"].as_str().unwrap_or("нет токена")))?;
    let expires = mc["expires_in"].as_u64().unwrap_or(86400);
    // 4. Профиль (проверка владения игрой)
    let pr = c
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(mc_token)
        .send()
        .await?;
    if pr.status().as_u16() == 404 {
        bail!("На этой учётной записи Microsoft не куплен Minecraft: Java Edition.");
    }
    let p: Value = pr.json().await?;
    Ok(Account {
        kind: AccountKind::Microsoft,
        name: p["name"].as_str().ok_or_else(|| anyhow!("нет имени профиля"))?.into(),
        uuid: p["id"].as_str().unwrap_or("").into(),
        access_token: mc_token.into(),
        refresh_token: refresh.into(),
        expires_at: now_secs() + expires.saturating_sub(120),
    })
}
