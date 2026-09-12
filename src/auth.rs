use crate::{
    config::Config,
    db::{Db, now},
};
use anyhow::Result;
use axum::http::HeaderMap;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub fn secret() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
pub fn hash(secret: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(secret.as_bytes()))
}
pub fn equal(a: &str, b: &str) -> bool {
    hash(a).as_bytes().ct_eq(hash(b).as_bytes()).into()
}
pub fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then(|| value.to_owned())
        })
}
pub fn set_cookie(config: &Config, name: &str, value: &str, age: i64) -> String {
    format!(
        "{name}={value}; Path=/; HttpOnly; SameSite=Lax; Max-Age={age}{}",
        if config.secure() { "; Secure" } else { "" }
    )
}
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub login: String,
    pub email: String,
}
pub async fn session(db: &Db, config: &Config, headers: &HeaderMap) -> Result<Option<User>> {
    let Some(token) = cookie(headers, "traces_session") else {
        return Ok(None);
    };
    let digest = hash(&token);
    let user = db
        .read(move |conn| {
            Ok(conn
                .query_row(
                    "SELECT github_id,login,email FROM sessions WHERE hash=? AND expires_at>?",
                    params![digest, now()],
                    |r| {
                        Ok(User {
                            id: r.get(0)?,
                            login: r.get(1)?,
                            email: r.get(2)?,
                        })
                    },
                )
                .optional()?)
        })
        .await?;
    Ok(user.filter(|u| config.allowed_emails.contains(&u.email.to_lowercase())))
}
pub async fn issue_session(db: &Db, user: User) -> Result<String> {
    let token = secret();
    let digest = hash(&token);
    db.write(move |conn| {
        conn.execute(
            "INSERT INTO sessions(hash,github_id,login,email,expires_at) VALUES(?,?,?,?,?)",
            params![
                digest,
                user.id,
                user.login,
                user.email,
                now() + 7 * 86400000
            ],
        )?;
        Ok(())
    })
    .await?;
    Ok(token)
}
// Some(None) is the legacy token; Some(Some(hash)) is a managed ingestion key.
pub async fn ingestion_key(
    db: &Db,
    config: &Config,
    headers: &HeaderMap,
) -> Result<Option<Option<String>>> {
    let Some(token) = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
    else {
        return Ok(None);
    };
    if config
        .legacy_token
        .as_ref()
        .is_some_and(|expected| equal(token, expected))
    {
        return Ok(Some(None));
    }
    let digest = hash(token);
    let key = digest.clone();
    let valid=db.read(move|conn|Ok(conn.query_row("SELECT 1 FROM api_keys WHERE hash=? AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at>?)",params![key,now()],|r|r.get::<_,i64>(0)).optional()?.is_some())).await?;
    Ok(valid.then_some(Some(digest)))
}
