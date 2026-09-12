use crate::{
    auth,
    config::Config,
    db::{Db, now},
    ingest::Ingest,
    model::Item,
    query::{self, Filters},
};
use anyhow::Result;
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tower_http::{
    compression::CompressionLayer,
    services::{ServeDir, ServeFile},
};

#[derive(Clone)]
pub struct App {
    pub db: Db,
    pub config: Arc<Config>,
    pub ingest: Ingest,
    client: reqwest::Client,
    maintenance_failed: Arc<AtomicBool>,
    request_slots: Arc<tokio::sync::Semaphore>,
}
impl App {
    pub fn new(db: Db, config: Config) -> Result<(Self, tokio::task::JoinHandle<()>)> {
        let (ingest, writer) = Ingest::start(db.clone(), config.queue_bytes);
        let client = reqwest::Client::builder()
            .user_agent("agent-traces/0.2")
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok((
            Self {
                db,
                config: Arc::new(config),
                ingest,
                client,
                maintenance_failed: Arc::new(AtomicBool::new(false)),
                request_slots: Arc::new(tokio::sync::Semaphore::new(8)),
            },
            writer,
        ))
    }
}
pub struct Error(StatusCode, String);
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let mut response = (self.0, Json(json!({"error":self.1}))).into_response();
        if self.0 == StatusCode::SERVICE_UNAVAILABLE {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, "2".parse().unwrap());
        }
        response
    }
}
impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error=%e,"Request failed");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Unable to complete request".into(),
        )
    }
}
fn bad(message: &str) -> Error {
    Error(StatusCode::BAD_REQUEST, message.into())
}
fn unauthorized() -> Error {
    Error(StatusCode::UNAUTHORIZED, "Sign in to continue".into())
}
fn missing() -> Error {
    Error(StatusCode::NOT_FOUND, "Trace or span not found".into())
}
type Api<T> = std::result::Result<T, Error>;

pub fn router(app: App) -> Router {
    let protected = Router::new()
        .route("/api/me", get(me))
        .route("/api/status", get(status))
        .route("/api/traces", get(list))
        .route("/api/traces/{id}", get(detail).delete(remove))
        .route("/api/traces/{id}/spans", get(spans))
        .route("/api/traces/{id}/spans/{span}", get(span))
        .route("/api/traces/{id}/search", get(search))
        .route("/api/traces/{id}/export", get(export))
        .route("/api/keys", get(keys).post(create_key))
        .route("/api/keys/{id}", delete(revoke_key))
        .route("/auth/logout", post(logout))
        .route_layer(middleware::from_fn_with_state(app.clone(), require_session));
    Router::new()
        .merge(protected)
        .route("/api/health", get(health))
        .route("/auth/config", get(auth_config))
        .route("/auth/github", get(login))
        .route("/auth/github/callback", get(callback))
        .merge(
            Router::new()
                .route("/v1/traces/ingest", post(ingest))
                .route_layer(middleware::from_fn_with_state(
                    app.clone(),
                    limit_ingest_requests,
                )),
        )
        .route(
            "/api/{*rest}",
            get(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error":"Unknown API endpoint"})),
                )
            }),
        )
        .fallback_service(
            ServeDir::new(&app.config.static_dir)
                .precompressed_gzip()
                .fallback(ServeFile::new(app.config.static_dir.join("200.html"))),
        )
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(security_headers))
        .with_state(app)
}
async fn limit_ingest_requests(
    State(app): State<App>,
    request: Request,
    next: Next,
) -> Api<Response> {
    let _permit = app.request_slots.clone().try_acquire_owned().map_err(|_| {
        Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Too many concurrent ingestion requests; retry later".into(),
        )
    })?;
    Ok(next.run(request).await)
}
async fn security_headers(request: Request, next: Next) -> Response {
    let api =
        request.uri().path().starts_with("/api/") || request.uri().path().starts_with("/auth/");
    let mut res = next.run(request).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("referrer-policy", "same-origin".parse().unwrap());
    h.insert("x-frame-options", "DENY".parse().unwrap());
    h.insert("content-security-policy","default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'".parse().unwrap());
    if api {
        h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    }
    res
}
async fn require_session(State(app): State<App>, mut req: Request, next: Next) -> Api<Response> {
    let user = auth::session(&app.db, &app.config, req.headers())
        .await?
        .ok_or_else(unauthorized)?;
    if !matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) {
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        let custom = req
            .headers()
            .get("x-traces-request")
            .and_then(|v| v.to_str().ok());
        if custom != Some("1") || origin.is_some_and(|o| o != app.config.public_url) {
            return Err(Error(
                StatusCode::FORBIDDEN,
                "Invalid request origin".into(),
            ));
        }
    }
    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}
async fn health(State(app): State<App>) -> (StatusCode, Json<Value>) {
    let healthy = !app.ingest.metrics.failed.load(Ordering::Relaxed)
        && !app.ingest.metrics.stopping.load(Ordering::Relaxed);
    (
        if healthy {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(json!({"ok":healthy})),
    )
}
async fn auth_config(State(app): State<App>) -> Json<Value> {
    Json(json!({"ready":app.config.oauth_ready()}))
}
async fn me(axum::Extension(user): axum::Extension<auth::User>) -> Json<auth::User> {
    Json(user)
}
async fn status(State(app): State<App>) -> Json<Value> {
    Json(
        json!({"pending":app.ingest.metrics.pending.load(Ordering::Relaxed),"queuedBytes":app.ingest.bytes_used(),"committed":app.ingest.metrics.committed.load(Ordering::Relaxed),"writerHealthy":!app.ingest.metrics.failed.load(Ordering::Relaxed),"maintenanceHealthy":!app.maintenance_failed.load(Ordering::Relaxed),"retentionDays":app.config.retention_days}),
    )
}
async fn ingest(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Api<Json<Value>> {
    let key = auth::ingestion_key(&app.db, &app.config, &headers)
        .await?
        .ok_or_else(|| Error(StatusCode::UNAUTHORIZED, "Invalid ingestion key".into()))?;
    let value: Value = serde_json::from_slice(&body).map_err(|_| bad("Invalid JSON body"))?;
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .filter(|a| a.len() <= 1000 && a.iter().all(Value::is_object))
        .ok_or_else(|| {
            bad("Invalid tracing ingest envelope: expected up to 1000 object records")
        })?;
    let items: Vec<Item> = data.iter().filter_map(Item::parse).collect();
    let accepted = items.len();
    let rejected = data.len() - accepted;
    app.ingest
        .enqueue(items, key)
        .map_err(|e| Error(StatusCode::SERVICE_UNAVAILABLE, e.to_string()))?;
    Ok(Json(json!({"accepted":accepted,"rejected":rejected})))
}
async fn list(State(app): State<App>, Query(filters): Query<Filters>) -> Api<Json<Value>> {
    if filters.q.as_ref().is_some_and(|s| s.len() > 512)
        || filters.cursor.as_ref().is_some_and(|s| s.len() > 2048)
    {
        return Err(bad("Search or cursor too long"));
    }
    // Validate cursors separately so client mistakes do not become server failures.
    if let Some(cursor) = &filters.cursor {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(cursor)
            .map_err(|_| bad("Invalid cursor"))?;
        serde_json::from_slice::<(i64, String)>(&bytes).map_err(|_| bad("Invalid cursor"))?;
    }
    Ok(Json(app.db.read(move |c| query::list(c, &filters)).await?))
}
async fn detail(State(app): State<App>, Path(id): Path<String>) -> Api<Json<Value>> {
    Ok(Json(
        app.db
            .read(move |c| query::trace(c, &id))
            .await?
            .ok_or_else(missing)?,
    ))
}
async fn spans(State(app): State<App>, Path(id): Path<String>) -> Api<Json<Value>> {
    Ok(Json(app.db.read(move |c| query::spans(c, &id)).await?))
}
async fn span(
    State(app): State<App>,
    Path((id, span)): Path<(String, String)>,
) -> Api<Json<Value>> {
    Ok(Json(
        app.db
            .read(move |c| query::span(c, &id, &span))
            .await?
            .ok_or_else(missing)?,
    ))
}
#[derive(Deserialize)]
struct Search {
    q: String,
}
async fn search(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(q): Query<Search>,
) -> Api<Json<Value>> {
    if q.q.trim().is_empty() || q.q.len() > 512 {
        return Err(bad("Search must be 1–512 bytes"));
    }
    Ok(Json(
        app.db.read(move |c| query::search(c, &id, &q.q)).await?,
    ))
}
async fn remove(State(app): State<App>, Path(id): Path<String>) -> Api<StatusCode> {
    let count = app
        .db
        .write(move |c| Ok(c.execute("DELETE FROM traces WHERE id=?", [id])?))
        .await?;
    if count == 0 {
        return Err(missing());
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn export(State(app): State<App>, Path(id): Path<String>) -> Api<Response> {
    let check = id.clone();
    if app
        .db
        .read(move |c| query::trace(c, &check))
        .await?
        .is_none()
    {
        return Err(missing());
    }
    let (tx, rx) = tokio::sync::mpsc::channel::<std::result::Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        let report = tx.clone();
        let result = app
            .db
            .read(move |c| {
                let snapshot = c.unchecked_transaction()?;
                let raw: String =
                    snapshot.query_row("SELECT raw FROM traces WHERE id=?", [&id], |r| r.get(0))?;
                tx.blocking_send(Ok(Bytes::from(format!("{{\"data\":[{raw}"))))?;
                let mut stmt = snapshot
                    .prepare("SELECT raw FROM spans WHERE trace_id=? ORDER BY started_at,id")?;
                let mut rows = stmt.query([id])?;
                while let Some(row) = rows.next()? {
                    let raw: String = row.get(0)?;
                    tx.blocking_send(Ok(Bytes::from(format!(",{raw}"))))?;
                }
                tx.blocking_send(Ok(Bytes::from_static(b"]}")))?;
                Ok(())
            })
            .await;
        if let Err(error) = result {
            tracing::warn!(%error,"Export interrupted");
            let _ = report
                .send(Err(std::io::Error::other("Export interrupted")))
                .await;
        }
    });
    Ok((
        [
            (header::CONTENT_TYPE, "application/json"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=trace.json",
            ),
        ],
        Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx)),
    )
        .into_response())
}
async fn keys(State(app): State<App>) -> Api<Json<Value>> {
    let rows=app.db.read(|c|{let mut stmt=c.prepare("SELECT id,name,prefix,created_at,expires_at,last_used_at,revoked_at FROM api_keys ORDER BY created_at DESC")?;
        Ok(stmt.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,"prefix":r.get::<_,String>(2)?,"createdAt":r.get::<_,i64>(3)?,"expiresAt":r.get::<_,Option<i64>>(4)?,"lastUsedAt":r.get::<_,Option<i64>>(5)?,"revokedAt":r.get::<_,Option<i64>>(6)?})))?.collect::<rusqlite::Result<Vec<_>>>()?)
    }).await?;
    Ok(Json(
        json!({"items":rows,"legacyEnabled":app.config.legacy_token.is_some()}),
    ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewKey {
    name: String,
    expires_at: Option<i64>,
}
async fn create_key(State(app): State<App>, Json(input): Json<NewKey>) -> Api<Json<Value>> {
    if input.name.trim().is_empty()
        || input.name.len() > 100
        || input.expires_at.is_some_and(|d| d <= now())
    {
        return Err(bad(
            "Name is required (up to 100 bytes); expiry must be in the future",
        ));
    }
    let token = format!("tr_{}", auth::secret());
    let digest = auth::hash(&token);
    let prefix = token[..11].to_owned();
    let id = auth::secret();
    let result_id = id.clone();
    app.db.write(move|c|{c.execute("INSERT INTO api_keys(id,name,prefix,hash,created_at,expires_at) VALUES(?,?,?,?,?,?)",params![id,input.name.trim(),prefix,digest,now(),input.expires_at])?;Ok(())}).await?;
    Ok(Json(json!({"id":result_id,"key":token})))
}
async fn revoke_key(State(app): State<App>, Path(id): Path<String>) -> Api<StatusCode> {
    let changed = app
        .db
        .write(move |c| {
            Ok(c.execute(
                "UPDATE api_keys SET revoked_at=COALESCE(revoked_at,?) WHERE id=?",
                params![now(), id],
            )?)
        })
        .await?;
    if changed == 0 {
        return Err(Error(StatusCode::NOT_FOUND, "API key not found".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
async fn login(State(app): State<App>) -> Api<Response> {
    if !app.config.oauth_ready() {
        return Err(Error(
            StatusCode::SERVICE_UNAVAILABLE,
            "Configure GitHub OAuth credentials and ALLOWED_EMAILS first".into(),
        ));
    }
    let state = auth::secret();
    let verifier = auth::secret();
    let challenge = auth::hash(&verifier);
    let digest = auth::hash(&state);
    app.db
        .write(move |c| {
            c.execute("DELETE FROM oauth_states WHERE expires_at<?", [now()])?;
            let count: i64 = c.query_row("SELECT COUNT(*) FROM oauth_states", [], |r| r.get(0))?;
            anyhow::ensure!(count < 1000, "Too many pending OAuth requests");
            c.execute(
                "INSERT INTO oauth_states(hash,verifier,expires_at) VALUES(?,?,?)",
                params![digest, verifier, now() + 600000],
            )?;
            Ok(())
        })
        .await?;
    let mut url = url::Url::parse(&app.config.github_authorize_url).map_err(anyhow::Error::from)?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", app.config.github_client_id.as_str()),
        (
            "redirect_uri",
            &format!("{}/auth/github/callback", app.config.public_url),
        ),
        ("scope", "read:user user:email"),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
    ]);
    Ok((
        [(
            header::SET_COOKIE,
            auth::set_cookie(&app.config, "traces_oauth", &state, 600),
        )],
        Redirect::temporary(url.as_str()),
    )
        .into_response())
}
#[derive(Deserialize)]
struct Callback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}
#[derive(Deserialize)]
struct Email {
    email: String,
    verified: bool,
}
async fn callback(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Api<Response> {
    if query.error.is_some() {
        return Ok(Redirect::to("/?auth_error=denied").into_response());
    }
    let code = query.code.ok_or_else(|| bad("Missing OAuth code"))?;
    let state = query.state.ok_or_else(|| bad("Missing OAuth state"))?;
    let cookie = auth::cookie(&headers, "traces_oauth")
        .ok_or_else(|| bad("OAuth cookie expired; try signing in again"))?;
    if !auth::equal(&state, &cookie) {
        return Err(bad("OAuth state mismatch"));
    }
    let digest = auth::hash(&state);
    let verifier = app
        .db
        .write(move |c| {
            Ok(c.query_row(
                "DELETE FROM oauth_states WHERE hash=? AND expires_at>? RETURNING verifier",
                params![digest, now()],
                |r| r.get::<_, String>(0),
            )
            .optional()?)
        })
        .await?
        .ok_or_else(|| bad("OAuth state expired or already used"))?;
    let token: Value = app
        .client
        .post(&app.config.github_token_url)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", app.config.github_client_id.as_str()),
            ("client_secret", app.config.github_client_secret.as_str()),
            ("code", &code),
            ("code_verifier", &verifier),
            (
                "redirect_uri",
                &format!("{}/auth/github/callback", app.config.public_url),
            ),
        ])
        .send()
        .await
        .map_err(anyhow::Error::from)?
        .error_for_status()
        .map_err(anyhow::Error::from)?
        .json()
        .await
        .map_err(anyhow::Error::from)?;
    let token = token
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("GitHub did not authorize this login"))?;
    let profile: Value = app
        .client
        .get(format!("{}/user", app.config.github_api_url))
        .bearer_auth(token)
        .send()
        .await
        .map_err(anyhow::Error::from)?
        .error_for_status()
        .map_err(anyhow::Error::from)?
        .json()
        .await
        .map_err(anyhow::Error::from)?;
    let mut email = None;
    for page in 1..=10 {
        let emails: Vec<Email> = app
            .client
            .get(format!(
                "{}/user/emails?per_page=100&page={page}",
                app.config.github_api_url
            ))
            .bearer_auth(token)
            .send()
            .await
            .map_err(anyhow::Error::from)?
            .error_for_status()
            .map_err(anyhow::Error::from)?
            .json()
            .await
            .map_err(anyhow::Error::from)?;
        email = emails
            .iter()
            .find(|e| e.verified && app.config.allowed_emails.contains(&e.email.to_lowercase()))
            .map(|e| e.email.to_lowercase());
        if email.is_some() || emails.len() < 100 {
            break;
        }
    }
    let Some(email) = email else {
        return Ok(Redirect::to("/?auth_error=not_allowed").into_response());
    };
    let user = auth::User {
        id: profile["id"]
            .as_i64()
            .ok_or_else(|| bad("Invalid GitHub identity"))?,
        login: profile["login"]
            .as_str()
            .ok_or_else(|| bad("Invalid GitHub identity"))?
            .into(),
        email,
    };
    let session = auth::issue_session(&app.db, user).await?;
    let mut response = Redirect::to("/traces").into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        auth::set_cookie(&app.config, "traces_session", &session, 7 * 86400)
            .parse()
            .unwrap(),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        auth::set_cookie(&app.config, "traces_oauth", "", 0)
            .parse()
            .unwrap(),
    );
    Ok(response)
}
async fn logout(State(app): State<App>, headers: HeaderMap) -> Api<Response> {
    if let Some(token) = auth::cookie(&headers, "traces_session") {
        let digest = auth::hash(&token);
        app.db
            .write(move |c| {
                c.execute("DELETE FROM sessions WHERE hash=?", [digest])?;
                Ok(())
            })
            .await?;
    }
    Ok((
        [(
            header::SET_COOKIE,
            auth::set_cookie(&app.config, "traces_session", "", 0),
        )],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}
pub async fn serve(config: Config, db: Db) -> Result<()> {
    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!(address=%config.bind,"Agent Traces listening");
    let (app, writer) = App::new(db, config)?;
    let maintenance = tokio::spawn(maintenance(app.clone()));
    let stop = app.ingest.clone();
    axum::serve(listener, router(app.clone()))
        .with_graceful_shutdown(async move {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("signal handler");
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
            }
            #[cfg(not(unix))]
            tokio::signal::ctrl_c().await.ok();
            stop.stop();
        })
        .await?;
    maintenance.abort();
    app.ingest.stop();
    tokio::time::timeout(Duration::from_secs(45), writer)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Shutdown could not drain queue: {} records remain",
                app.ingest.metrics.pending.load(Ordering::Relaxed)
            )
        })??;
    Ok(())
}
async fn maintenance(app: App) {
    let mut timer = tokio::time::interval(Duration::from_secs(3600));
    let mut last_backup = std::fs::read_dir(&app.config.backup_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "sqlite3"))
        .filter_map(|e| {
            e.metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
        })
        .map(|d| d.as_millis() as i64)
        .max()
        .unwrap_or(0);
    loop {
        timer.tick().await;
        let result = app
            .db
            .write(|c| {
                c.execute("DELETE FROM sessions WHERE expires_at<?", [now()])?;
                c.execute("DELETE FROM oauth_states WHERE expires_at<?", [now()])?;
                Ok(())
            })
            .await;
        let mut failed = result.is_err();
        if let Err(error) = result {
            tracing::error!(%error, "Session cleanup failed");
        }
        if app.config.retention_days > 0 {
            let cutoff = now() - i64::from(app.config.retention_days) * 86400000;
            loop {
                match app
                    .db
                    .write(move |c| crate::db::expire_batch(c, cutoff))
                    .await
                {
                    Ok(0) => break,
                    Ok(_) => tokio::time::sleep(Duration::from_millis(10)).await,
                    Err(error) => {
                        failed = true;
                        tracing::error!(%error,"Retention cleanup failed");
                        break;
                    }
                }
            }
        }
        if now() - last_backup >= 86400000 {
            match app.db.backup(app.config.backup_dir.clone()).await {
                Ok(path) => {
                    last_backup = now();
                    tracing::info!(path=%path.display(), "Backup complete");
                }
                Err(error) => {
                    failed = true;
                    tracing::error!(%error, "Backup failed");
                }
            }
        }
        app.maintenance_failed.store(failed, Ordering::Relaxed);
    }
}
