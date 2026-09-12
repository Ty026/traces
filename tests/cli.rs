use serde_json::json;
use std::{
    process::{Command, Stdio},
    time::Duration,
};
#[tokio::test]
async fn singleton_online_backup_graceful_drain_and_restore() {
    let dir = tempfile::tempdir().unwrap();
    let database = dir.path().join("traces.sqlite3");
    let backups = dir.path().join("backups");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let command = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_agent-traces"));
        c.current_dir(dir.path())
            .env("DATABASE_PATH", &database)
            .env("BACKUP_DIR", &backups)
            .env("BIND_ADDR", addr.to_string())
            .env("PUBLIC_BASE_URL", format!("http://{addr}"))
            .env("ALLOWED_EMAILS", "")
            .env("TRACE_INGEST_TOKEN", "cli-test-token")
            .env("GITHUB_CLIENT_ID", "")
            .env("GITHUB_CLIENT_SECRET", "");
        c
    };
    let mut server = command()
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let client = reqwest::Client::new();
    let ready = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if client
                .get(format!("http://{addr}/api/health"))
                .send()
                .await
                .is_ok()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    if ready.is_err() {
        let _ = server.kill();
        panic!("Server did not start")
    }
    let duplicate = command().output().unwrap();
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("Another instance"));
    let online = command().arg("backup").output().unwrap();
    assert!(
        online.status.success(),
        "{}",
        String::from_utf8_lossy(&online.stderr)
    );
    let response = client
        .post(format!("http://{addr}/v1/traces/ingest"))
        .bearer_auth("cli-test-token")
        .json(&json!({"data":[{"object":"trace","id":"cli-test","workflow_name":"Drain me"}]}))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    // Terminate immediately after acknowledgement; normal shutdown must drain the queue.
    #[cfg(unix)]
    {
        assert!(
            Command::new("kill")
                .args(["-TERM", &server.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
    }
    #[cfg(not(unix))]
    {
        server.kill().unwrap();
    }
    assert!(server.wait().unwrap().success());
    let conn = agent_traces::db::connect(&database, false).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM traces WHERE id='cli-test'", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(conn);
    let backup = command().arg("backup").output().unwrap();
    assert!(
        backup.status.success(),
        "{}",
        String::from_utf8_lossy(&backup.stderr)
    );
    let path = String::from_utf8(backup.stdout).unwrap().trim().to_owned();
    let conn = agent_traces::db::connect(&database, false).unwrap();
    conn.execute("DELETE FROM traces", []).unwrap();
    drop(conn);
    let restored = command().args(["restore", &path]).output().unwrap();
    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    let conn = agent_traces::db::connect(&database, true).unwrap();
    assert_eq!(
        conn.query_row("SELECT COUNT(*) FROM traces WHERE id='cli-test'", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let unrelated = dir.path().join("unrelated.sqlite3");
    rusqlite::Connection::open(&unrelated).unwrap();
    assert!(
        !command()
            .arg("restore")
            .arg(unrelated)
            .output()
            .unwrap()
            .status
            .success()
    );
}
