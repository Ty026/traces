use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct Db {
    writer: Arc<Mutex<Connection>>,
    readers: Arc<Mutex<Vec<Connection>>>,
    permits: Arc<Semaphore>,
    pub path: PathBuf,
}
impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut writer = connect(path, false)?;
        writer.pragma_update(None, "journal_mode", "WAL")?;
        writer.pragma_update(None, "synchronous", "NORMAL")?;
        let version: i64 = writer.pragma_query_value(None, "user_version", |r| r.get(0))?;
        anyhow::ensure!(version <= 2, "Database was created by a newer application");
        let tx = writer.transaction()?;
        if version == 0 {
            tx.execute_batch(include_str!("../migrations/001.sql"))?;
        }
        if version < 2 {
            tx.execute_batch(include_str!("../migrations/002.sql"))?;
        }
        tx.commit()?;
        let mut readers = Vec::new();
        for _ in 0..4 {
            readers.push(connect(path, true)?);
        }
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            readers: Arc::new(Mutex::new(readers)),
            permits: Arc::new(Semaphore::new(4)),
            path: path.into(),
        })
    }
    pub async fn write<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let writer = self.writer.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = writer
                .lock()
                .map_err(|_| anyhow::anyhow!("Database writer unavailable"))?;
            f(&mut conn)
        })
        .await?
    }
    pub async fn read<T, F>(&self, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
    {
        let permit = self.permits.clone().acquire_owned().await?;
        let readers = self.readers.clone();
        tokio::task::spawn_blocking(move || {
            let conn = readers
                .lock()
                .unwrap()
                .pop()
                .context("No reader available")?;
            let result = f(&conn);
            readers.lock().unwrap().push(conn);
            drop(permit);
            result
        })
        .await?
    }
    pub async fn backup(&self, directory: PathBuf) -> Result<PathBuf> {
        let path = self.path.clone();
        tokio::task::spawn_blocking(move || backup(&path, &directory)).await?
    }
}
pub fn connect(path: &Path, readonly: bool) -> Result<Connection> {
    let flags = if readonly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE
    };
    let conn = Connection::open_with_flags(path, flags | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.pragma_update(None, "cache_size", -8192)?;
    Ok(conn)
}
pub fn backup(path: &Path, directory: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    let name = format!(
        "traces-{}.sqlite3",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.9fZ")
    );
    let target = directory.join(name);
    let temporary = target.with_extension("partial");
    let conn = connect(path, true)?;
    conn.backup(rusqlite::MAIN_DB, &temporary, None)?;
    let check = connect(&temporary, false)?;
    check.pragma_update(None, "journal_mode", "DELETE")?;
    let result: String = check.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    anyhow::ensure!(result == "ok", "Backup integrity check failed");
    drop(check);
    std::fs::rename(&temporary, &target)?;
    let mut backups: Vec<PathBuf> = std::fs::read_dir(directory)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("traces-"))
                && p.extension().is_some_and(|e| e == "sqlite3")
        })
        .collect();
    backups.sort();
    for old in backups.iter().take(backups.len().saturating_sub(7)) {
        std::fs::remove_file(old)?;
    }
    Ok(target)
}
pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// Deletes a bounded number of complete traces; callers yield between batches.
pub fn expire_batch(conn: &mut Connection, cutoff: i64) -> Result<usize> {
    let tx = conn.transaction()?;
    let count = tx.execute(
        "DELETE FROM traces WHERE id IN(SELECT id FROM traces WHERE last_seen<? LIMIT 100)",
        [cutoff],
    )?;
    tx.commit()?;
    Ok(count)
}
