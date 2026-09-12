use agent_traces::{
    config::Config,
    db::{self, Db},
    server,
};
use anyhow::{Context, Result};
use fs2::FileExt;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agent_traces=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env()?;
    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    if command == "backup" {
        println!(
            "{}",
            db::backup(&config.database, &config.backup_dir)?.display()
        );
        return Ok(());
    }
    anyhow::ensure!(
        command == "serve" || command == "restore",
        "Usage: agent-traces [serve|backup|restore BACKUP_FILE]"
    );
    if let Some(parent) = config.database.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(config.database.with_extension("lock"))?;
    lock.try_lock_exclusive().context("Another instance is using this database. Stop the service before restoring or starting another instance.")?;
    if command == "restore" {
        let source = std::env::args()
            .nth(2)
            .context("Usage: agent-traces restore BACKUP_FILE (service must be stopped)")?;
        let source = std::path::Path::new(&source);
        anyhow::ensure!(source.is_file(), "Backup file not found");
        let source_conn = db::connect(source, true)?;
        let check: String = source_conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        anyhow::ensure!(check == "ok", "Backup integrity check failed");
        let version: i64 = source_conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        anyhow::ensure!((1..=2).contains(&version), "Unsupported backup schema");
        anyhow::ensure!(
            source.canonicalize()? != config.database.canonicalize().unwrap_or_default(),
            "Backup must differ from database"
        );
        if config.database.exists() {
            db::backup(&config.database, &config.backup_dir)?;
        }
        let mut target = db::connect(&config.database, false)?;
        rusqlite::backup::Backup::new(&source_conn, &mut target)?.run_to_completion(
            100,
            std::time::Duration::from_millis(5),
            None,
        )?;
        // Restored browser sessions must not resurrect previously logged-out access.
        target.execute("DELETE FROM sessions", [])?;
        target.execute("DELETE FROM oauth_states", [])?;
        println!(
            "Restored {}. Browser sessions invalidated; review restored API keys before restarting.",
            config.database.display()
        );
    } else {
        let db = Db::open(&config.database)?;
        server::serve(config, db).await?;
    }
    drop(lock);
    Ok(())
}
