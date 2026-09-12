use agent_traces::{
    auth,
    config::Config,
    db::{Db, now},
    ingest::{Ingest, persist_items, refresh_bounds},
    model::Item,
    query,
    server::{App, router},
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{collections::HashSet, sync::atomic::Ordering, time::Duration};
use tower::ServiceExt;

fn config(dir: &tempfile::TempDir) -> Config {
    Config {
        bind: "127.0.0.1:0".into(),
        database: dir.path().join("db.sqlite3"),
        backup_dir: dir.path().join("backups"),
        static_dir: dir.path().join("static"),
        public_url: "http://localhost:3000".into(),
        github_client_id: "test-client".into(),
        github_client_secret: "test-secret".into(),
        allowed_emails: HashSet::from(["admin@example.com".into()]),
        legacy_token: Some("legacy-test-token".into()),
        retention_days: 30,
        queue_bytes: 1024 * 1024,
        github_authorize_url: "https://github.com/login/oauth/authorize".into(),
        github_token_url: "https://github.com/login/oauth/access_token".into(),
        github_api_url: "https://api.github.com".into(),
    }
}
struct Harness {
    _dir: tempfile::TempDir,
    app: App,
    router: Router,
    session: String,
    writer: tokio::task::JoinHandle<()>,
}
impl Harness {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir);
        let db = Db::open(&config.database).unwrap();
        let session = auth::issue_session(
            &db,
            auth::User {
                id: 1,
                login: "tester".into(),
                email: "admin@example.com".into(),
            },
        )
        .await
        .unwrap();
        let (app, writer) = App::new(db, config).unwrap();
        let router = router(app.clone());
        Self {
            _dir: dir,
            app,
            router,
            session,
            writer,
        }
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: Value,
        auth_kind: &str,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("Content-Type", "application/json")
            .header("X-Traces-Request", "1")
            .header("Origin", "http://localhost:3000");
        if auth_kind == "session" {
            request = request.header("Cookie", format!("traces_session={}", self.session));
        } else if !auth_kind.is_empty() {
            request = request.header("Authorization", format!("Bearer {auth_kind}"));
        }
        let response = self
            .router
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
    async fn drain(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.app.ingest.metrics.pending.load(Ordering::Relaxed) > 0 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn stop(self) {
        self.app.ingest.stop();
        self.writer.await.unwrap();
    }
}
fn span(id: &str, trace: &str) -> Value {
    json!({"object":"trace.span","id":id,"trace_id":trace,"started_at":"2026-09-12T10:00:00Z","ended_at":"2026-09-12T10:00:01Z","span_data":{"type":"generation","model":"model-a","input":[{"role":"user","content":"searchable secret prompt"}],"output":[{"role":"assistant","content":"A result"}],"usage":{"input_tokens":10,"output_tokens":20}}})
}
#[tokio::test]
async fn ingestion_contract_upserts_out_of_order_and_lazy_payloads() {
    let h = Harness::new().await;
    for value in [
        json!({}),
        json!({"data":[1]}),
        json!({"data":vec![json!({});1001]}),
    ] {
        assert_eq!(
            h.request("POST", "/v1/traces/ingest", value, "legacy-test-token")
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let first = span("span-1", "trace-1");
    let batch = json!({"data":[first,{"object":"invalid"},{"object":"trace.span","id":"bad"}]});
    assert_eq!(
        h.request("POST", "/v1/traces/ingest", batch, "legacy-test-token")
            .await,
        (StatusCode::OK, json!({"accepted":1,"rejected":2}))
    );
    h.drain().await;
    let (_, detail) = h
        .request("GET", "/api/traces/trace-1", Value::Null, "session")
        .await;
    assert_eq!(detail["workflowName"], "Unknown workflow");
    assert_eq!(detail["spanCount"], 1);
    let (_, tree) = h
        .request("GET", "/api/traces/trace-1/spans", Value::Null, "session")
        .await;
    assert!(!tree.to_string().contains("secret prompt"));
    assert_eq!(tree["items"][0]["inputTokens"], 10);
    let (_, body) = h
        .request(
            "GET",
            "/api/traces/trace-1/spans/span-1",
            Value::Null,
            "session",
        )
        .await;
    assert_eq!(body, first);
    let mut updated = first.clone();
    updated["error"] = json!({"message":"timeout"});
    updated["ended_at"] = Value::Null;
    h.request("POST","/v1/traces/ingest",json!({"data":[{"object":"trace","id":"trace-1","workflow_name":"Research","metadata":{"team":"test"}},updated.clone(),updated]}),"legacy-test-token").await;
    h.drain().await;
    let (_, detail) = h
        .request("GET", "/api/traces/trace-1", Value::Null, "session")
        .await;
    assert_eq!(detail["spanCount"], 1);
    assert_eq!(detail["errorCount"], 1);
    assert_eq!(detail["unfinishedCount"], 1);
    assert_eq!(detail["workflowName"], "Research");
    assert_eq!(detail["endedAt"], Value::Null);
    let (_, search) = h
        .request(
            "GET",
            "/api/traces/trace-1/search?q=secret",
            Value::Null,
            "session",
        )
        .await;
    assert_eq!(search["ids"], json!(["span-1"]));
    let (_, export) = h
        .request("GET", "/api/traces/trace-1/export", Value::Null, "session")
        .await;
    assert_eq!(export["data"].as_array().unwrap().len(), 2);
    let moved = span("span-1", "trace-2");
    h.request(
        "POST",
        "/v1/traces/ingest",
        json!({"data":[moved]}),
        "legacy-test-token",
    )
    .await;
    h.drain().await;
    let (_, old) = h
        .request("GET", "/api/traces/trace-1", Value::Null, "session")
        .await;
    assert_eq!(old["spanCount"], 0);
    assert_eq!(old["errorCount"], 0);
    assert_eq!(old["startedAt"], Value::Null);
    assert_eq!(
        h.request("DELETE", "/api/traces/trace-2", Value::Null, "session")
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        h.request(
            "GET",
            "/api/traces/trace-2/spans/span-1",
            Value::Null,
            "session"
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    h.stop().await;
}
#[tokio::test]
async fn keys_scopes_expiry_revocation_csrf_and_logout() {
    let h = Harness::new().await;
    for path in [
        "/api/me",
        "/api/traces",
        "/api/status",
        "/api/keys",
        "/api/traces/a/export",
    ] {
        assert_eq!(
            h.request("GET", path, Value::Null, "").await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            h.request("GET", path, Value::Null, "legacy-test-token")
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        h.request("POST", "/v1/traces/ingest", json!({"data":[]}), "session")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, key) = h
        .request("POST", "/api/keys", json!({"name":"test"}), "session")
        .await;
    let token = key["key"].as_str().unwrap();
    assert_eq!(
        h.request(
            "POST",
            "/v1/traces/ingest",
            json!({"data":[span("s","t")]}),
            token
        )
        .await
        .0,
        StatusCode::OK
    );
    h.drain().await;
    let (_, listing) = h.request("GET", "/api/keys", Value::Null, "session").await;
    assert!(!listing.to_string().contains(token));
    assert!(listing["items"][0]["lastUsedAt"].as_i64().is_some());
    let headers = Request::builder()
        .method("DELETE")
        .uri(format!("/api/keys/{}", key["id"].as_str().unwrap()))
        .header("Cookie", format!("traces_session={}", h.session))
        .header("Origin", "https://evil.example")
        .header("X-Traces-Request", "1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        h.router.clone().oneshot(headers).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        h.request(
            "DELETE",
            &format!("/api/keys/{}", key["id"].as_str().unwrap()),
            Value::Null,
            "session"
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        h.request("POST", "/v1/traces/ingest", json!({"data":[]}), token)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let expired = auth::hash("expired");
    h.app.db.write(move|c|{c.execute("INSERT INTO api_keys(id,name,prefix,hash,created_at,expires_at) VALUES('expired','expired','ex',?,0,1)",[expired])?;Ok(())}).await.unwrap();
    assert_eq!(
        h.request("POST", "/v1/traces/ingest", json!({"data":[]}), "expired")
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    h.request("POST", "/auth/logout", Value::Null, "session")
        .await;
    assert_eq!(
        h.request("GET", "/api/me", Value::Null, "session").await.0,
        StatusCode::UNAUTHORIZED
    );
    h.stop().await;
}
#[tokio::test]
async fn bounded_queue_failure_recovery_and_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("db")).unwrap();
    let (ingest, worker) = Ingest::start(db.clone(), 1024);
    assert!(
        ingest
            .enqueue(vec![Item::parse(&span("s", "t")).unwrap()], None)
            .is_err()
    );
    assert_eq!(ingest.metrics.pending.load(Ordering::Relaxed), 0);
    ingest.stop();
    worker.await.unwrap();
    // Force a real database failure after admission, verify batch retention, then recover.
    let (ingest, worker) = Ingest::start(db.clone(), 1024 * 1024);
    db.write(|c|{c.execute_batch("CREATE TRIGGER fail_writes BEFORE INSERT ON spans BEGIN SELECT RAISE(ABORT,'test write failure'); END;")?;Ok(())}).await.unwrap();
    ingest
        .enqueue(vec![Item::parse(&span("s", "t")).unwrap()], None)
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !ingest.metrics.failed.load(Ordering::Relaxed) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(ingest.metrics.pending.load(Ordering::Relaxed), 1);
    assert!(ingest.bytes_used() > 0);
    assert!(
        ingest
            .enqueue(vec![Item::parse(&span("other", "t")).unwrap()], None)
            .is_err()
    );
    db.write(|c| {
        c.execute_batch("DROP TRIGGER fail_writes")?;
        Ok(())
    })
    .await
    .unwrap();
    ingest.stop();
    tokio::time::timeout(Duration::from_secs(4), worker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ingest.metrics.pending.load(Ordering::Relaxed), 0);
    assert_eq!(ingest.bytes_used(), 0);
    assert!(
        db.read(|c| query::span(c, "t", "s"))
            .await
            .unwrap()
            .is_some()
    );
}
#[tokio::test]
async fn keyset_filters_backup_and_allowlist_changes() {
    let h = Harness::new().await;
    h.app
        .db
        .write(|c| {
            let tx = c.transaction()?;
            let mut touched = HashSet::new();
            for i in 0..120 {
                let mut value = span(&format!("s-{i:03}"), &format!("t-{i:03}"));
                if i % 3 == 0 {
                    value["error"] = json!({"message":"timeout"});
                }
                persist_items(&tx, &[Item::parse(&value).unwrap()], now(), &mut touched)?;
            }
            refresh_bounds(&tx, touched)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
    let (_, first) = h
        .request("GET", "/api/traces?limit=100", Value::Null, "session")
        .await;
    assert_eq!(first["items"].as_array().unwrap().len(), 100);
    let cursor = first["nextCursor"].as_str().unwrap();
    let (_, second) = h
        .request(
            "GET",
            &format!("/api/traces?limit=100&cursor={cursor}"),
            Value::Null,
            "session",
        )
        .await;
    assert_eq!(second["items"].as_array().unwrap().len(), 20);
    let mut ids = HashSet::new();
    for row in first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
    {
        assert!(ids.insert(row["id"].as_str().unwrap()));
    }
    for filter in ["status=errors", "q=timeout"] {
        let (_, data) = h
            .request(
                "GET",
                &format!("/api/traces?limit=100&{filter}"),
                Value::Null,
                "session",
            )
            .await;
        assert_eq!(data["items"].as_array().unwrap().len(), 40);
    }
    let (_, none) = h
        .request("GET", "/api/traces?q=%25", Value::Null, "session")
        .await;
    assert!(none["items"].as_array().unwrap().is_empty());
    assert_eq!(
        h.request("GET", "/api/traces?cursor=bad", Value::Null, "session")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let backup = h
        .app
        .db
        .backup(h.app.config.backup_dir.clone())
        .await
        .unwrap();
    let conn = agent_traces::db::connect(&backup, true).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM spans", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        120
    );
    let mut config = (*h.app.config).clone();
    config.allowed_emails.clear();
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "cookie",
        format!("traces_session={}", h.session).parse().unwrap(),
    );
    assert!(
        auth::session(&h.app.db, &config, &headers)
            .await
            .unwrap()
            .is_none()
    );
    h.stop().await;
}

#[tokio::test]
async fn oauth_verified_email_pkce_state_replay_and_denial() {
    use axum::{
        Form, Json,
        routing::{get, post},
    };
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    let challenge = Arc::new(Mutex::new(String::new()));
    let verified = Arc::new(AtomicBool::new(true));
    let expect_challenge = challenge.clone();
    let email_verified = verified.clone();
    let provider=Router::new().route("/token",post(move|Form(form):Form<std::collections::HashMap<String,String>>|{let expected=expect_challenge.clone();async move{assert_eq!(form["client_id"],"test-client");assert_eq!(form["client_secret"],"test-secret");assert_eq!(auth::hash(&form["code_verifier"]),*expected.lock().unwrap());Json(json!({"access_token":"controlled-provider-token"}))}}))
    .route("/user",get(||async{Json(json!({"id":42,"login":"tester"}))}))
    .route("/user/emails",get(move||{let valid=email_verified.clone();async move{Json(json!([{"email":"private@example.com","verified":true},{"email":"ADMIN@example.com","verified":valid.load(Ordering::Relaxed)}]))}}));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let provider_task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let dir = tempfile::tempdir().unwrap();
    let mut config = config(&dir);
    config.github_token_url = format!("http://{address}/token");
    config.github_api_url = format!("http://{address}");
    let db = Db::open(&config.database).unwrap();
    let (app, writer) = App::new(db, config).unwrap();
    let router = router(app.clone());
    for allowed in [true, false] {
        verified.store(allowed, Ordering::Relaxed);
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/auth/github")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        let location = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
        let params: std::collections::HashMap<_, _> = location.query_pairs().into_owned().collect();
        *challenge.lock().unwrap() = params["code_challenge"].clone();
        assert_eq!(params["code_challenge_method"], "S256");
        let cookie = response.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        let callback = format!(
            "/auth/github/callback?code=test-code&state={}",
            params["state"]
        );
        let wrong = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&callback)
                    .header("Cookie", "traces_oauth=wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&callback)
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        if allowed {
            assert_eq!(response.headers()["location"], "/traces");
            let session = response
                .headers()
                .get_all("set-cookie")
                .iter()
                .find_map(|h| {
                    let h = h.to_str().ok()?;
                    h.starts_with("traces_session=").then_some(h.to_owned())
                })
                .unwrap();
            assert!(session.contains("HttpOnly"));
            assert!(session.contains("SameSite=Lax"));
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/me")
                        .header("Cookie", session.split(';').next().unwrap())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        } else {
            assert_eq!(response.headers()["location"], "/?auth_error=not_allowed");
            assert!(response.headers().get("set-cookie").is_none());
        }
        let replay = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(&callback)
                    .header("Cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::BAD_REQUEST);
    }
    app.ingest.stop();
    writer.await.unwrap();
    provider_task.abort();
}

#[tokio::test]
async fn retention_uses_last_receipt_and_backups_rotate() {
    let h = Harness::new().await;
    h.app
        .db
        .write(|c| {
            let tx = c.transaction()?;
            let mut touched = HashSet::new();
            persist_items(
                &tx,
                &[Item::parse(&span("old", "old")).unwrap()],
                1,
                &mut touched,
            )?;
            persist_items(
                &tx,
                &[Item::parse(&span("active", "active")).unwrap()],
                now(),
                &mut touched,
            )?;
            refresh_bounds(&tx, touched)?;
            tx.commit()?;
            assert_eq!(agent_traces::db::expire_batch(c, now() - 86400000)?, 1);
            Ok(())
        })
        .await
        .unwrap();
    assert!(
        h.app
            .db
            .read(|c| query::trace(c, "old"))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        h.app
            .db
            .read(|c| query::trace(c, "active"))
            .await
            .unwrap()
            .is_some()
    );
    for _ in 0..9 {
        h.app
            .db
            .backup(h.app.config.backup_dir.clone())
            .await
            .unwrap();
    }
    assert_eq!(
        std::fs::read_dir(&h.app.config.backup_dir).unwrap().count(),
        7
    );
    h.stop().await;
}

#[tokio::test]
async fn normalized_search_updates_and_v1_migration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("legacy.sqlite3");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(include_str!("../migrations/001.sql"))
        .unwrap();
    conn.execute("INSERT INTO traces(id,workflow_name,raw,first_seen,last_seen) VALUES('old','Legacy workflow','{}',1,1)",[]).unwrap();
    drop(conn);
    let db = Db::open(&path).unwrap();
    let found = db
        .read(|c| {
            query::list(
                c,
                &query::Filters {
                    q: Some("Legacy".into()),
                    ..Default::default()
                },
            )
        })
        .await
        .unwrap();
    assert_eq!(found["items"].as_array().unwrap().len(), 1);
    let item=Item::parse(&json!({"object":"trace","id":"old","workflow_name":"a\"b 100%_实验","group_id":"group-new"})).unwrap();
    db.write(move |c| {
        let tx = c.transaction()?;
        let mut touched = HashSet::new();
        persist_items(&tx, &[item], now(), &mut touched)?;
        refresh_bounds(&tx, touched)?;
        tx.commit()?;
        Ok(())
    })
    .await
    .unwrap();
    for term in ["a\"b", "100%_", "实验", "group-new"] {
        let q = term.to_owned();
        let result = db
            .read(move |c| {
                query::list(
                    c,
                    &query::Filters {
                        q: Some(q),
                        ..Default::default()
                    },
                )
            })
            .await
            .unwrap();
        assert_eq!(result["items"].as_array().unwrap().len(), 1, "{term}");
    }
    let found = db
        .read(|c| {
            query::list(
                c,
                &query::Filters {
                    q: Some("Legacy".into()),
                    ..Default::default()
                },
            )
        })
        .await
        .unwrap();
    assert!(found["items"].as_array().unwrap().is_empty());
    db.write(|c| {
        c.execute("DELETE FROM traces WHERE id='old'", [])?;
        let count: i64 = c.query_row("SELECT COUNT(*) FROM trace_search", [], |r| r.get(0))?;
        assert_eq!(count, 0);
        Ok(())
    })
    .await
    .unwrap();
}
