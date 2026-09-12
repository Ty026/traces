use crate::{
    db::{Db, now},
    model::{Item, Kind},
};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};

pub struct Batch {
    pub items: Vec<Item>,
    pub received: i64,
    pub key_hash: Option<String>,
    _permit: OwnedSemaphorePermit,
}
#[derive(Default)]
pub struct Metrics {
    pub pending: AtomicUsize,
    pub committed: AtomicU64,
    pub failed: AtomicBool,
    pub stopping: AtomicBool,
}
#[derive(Clone)]
pub struct Ingest {
    sender: mpsc::Sender<Batch>,
    budget: Arc<Semaphore>,
    pub metrics: Arc<Metrics>,
    pub capacity: u32,
}
impl Ingest {
    pub fn start(db: Db, bytes: u32) -> (Self, tokio::task::JoinHandle<()>) {
        let (sender, receiver) = mpsc::channel(256);
        let metrics = Arc::new(Metrics::default());
        let ingest = Self {
            sender,
            budget: Arc::new(Semaphore::new(bytes as usize)),
            metrics: metrics.clone(),
            capacity: bytes,
        };
        let task = tokio::spawn(worker(db, receiver, metrics));
        (ingest, task)
    }
    pub fn enqueue(&self, items: Vec<Item>, key_hash: Option<String>) -> Result<()> {
        if self.metrics.failed.load(Ordering::Relaxed)
            || self.metrics.stopping.load(Ordering::Relaxed)
        {
            anyhow::bail!("Writer unavailable; retry later");
        }
        if items.is_empty() {
            return Ok(());
        }
        let bytes: usize = items.iter().map(Item::bytes).sum();
        let bytes =
            u32::try_from(bytes).map_err(|_| anyhow::anyhow!("Batch exceeds queue budget"))?;
        let permit = self
            .budget
            .clone()
            .try_acquire_many_owned(bytes)
            .map_err(|_| anyhow::anyhow!("Queue full; retry later"))?;
        let count = items.len();
        // Account before send: the worker may receive and commit immediately.
        self.metrics.pending.fetch_add(count, Ordering::Relaxed);
        if self
            .sender
            .try_send(Batch {
                items,
                received: now(),
                key_hash,
                _permit: permit,
            })
            .is_err()
        {
            self.metrics.pending.fetch_sub(count, Ordering::Relaxed);
            anyhow::bail!("Queue full; retry later");
        }
        Ok(())
    }
    pub fn bytes_used(&self) -> usize {
        self.capacity as usize - self.budget.available_permits()
    }
    pub fn stop(&self) {
        self.metrics.stopping.store(true, Ordering::Relaxed);
    }
}
async fn worker(db: Db, mut receiver: mpsc::Receiver<Batch>, metrics: Arc<Metrics>) {
    let mut interval = tokio::time::interval(Duration::from_millis(200));
    loop {
        interval.tick().await;
        if metrics.stopping.load(Ordering::Relaxed) {
            receiver.close();
        }
        let mut batches = Vec::new();
        let mut count = 0;
        while count < 2000 {
            match receiver.try_recv() {
                Ok(batch) => {
                    count += batch.items.len();
                    batches.push(batch);
                }
                Err(_) => break,
            }
        }
        if batches.is_empty() {
            if receiver.is_closed() {
                break;
            }
            continue;
        }
        // Preserve ownership across retries, including the memory-budget permits.
        let batches = Arc::new(batches);
        loop {
            let work = batches.clone();
            match db.write(move |conn| persist(conn, &work)).await {
                Ok(()) => {
                    metrics.failed.store(false, Ordering::Relaxed);
                    metrics.pending.fetch_sub(count, Ordering::Relaxed);
                    metrics.committed.fetch_add(count as u64, Ordering::Relaxed);
                    break;
                }
                Err(error) => {
                    metrics.failed.store(true, Ordering::Relaxed);
                    tracing::error!(%error, "SQLite write failed; retaining batch and refusing new ingestion");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}
fn persist(conn: &mut Connection, batches: &[Batch]) -> Result<()> {
    let tx = conn.transaction()?;
    let mut touched = HashSet::new();
    for batch in batches {
        persist_items(&tx, &batch.items, batch.received, &mut touched)?;
        if let Some(hash) = &batch.key_hash {
            tx.execute(
                "UPDATE api_keys SET last_used_at=? WHERE hash=?",
                params![batch.received, hash],
            )?;
        }
    }
    refresh_bounds(&tx, touched)?;
    tx.commit()?;
    Ok(())
}
// Shared by ingestion and realistic benchmark seeding; callers own the transaction.
pub fn persist_items(
    conn: &Connection,
    items: &[Item],
    received: i64,
    touched: &mut HashSet<String>,
) -> Result<()> {
    let mut trace_insert = conn.prepare_cached("INSERT INTO traces(id,workflow_name,group_id,metadata,raw,first_seen,last_seen) VALUES(?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET workflow_name=excluded.workflow_name,group_id=excluded.group_id,metadata=excluded.metadata,raw=excluded.raw,last_seen=MAX(traces.last_seen,excluded.last_seen)")?;
    let mut placeholder = conn.prepare_cached("INSERT INTO traces(id,workflow_name,raw,first_seen,last_seen) VALUES(?,'Unknown workflow',?,?,?) ON CONFLICT(id) DO UPDATE SET last_seen=MAX(traces.last_seen,excluded.last_seen)")?;
    let mut span_insert = conn.prepare_cached("INSERT INTO spans(id,trace_id,parent_id,span_type,name,model,started_at,ended_at,duration_ms,has_error,error_text,input_tokens,output_tokens,raw) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET trace_id=excluded.trace_id,parent_id=excluded.parent_id,span_type=excluded.span_type,name=excluded.name,model=excluded.model,started_at=excluded.started_at,ended_at=excluded.ended_at,duration_ms=excluded.duration_ms,has_error=excluded.has_error,error_text=excluded.error_text,input_tokens=excluded.input_tokens,output_tokens=excluded.output_tokens,raw=excluded.raw")?;
    for item in items {
        match &item.kind {
            Kind::Trace {
                id,
                workflow,
                group,
                metadata,
            } => {
                trace_insert.execute(params![
                    id, workflow, group, metadata, item.raw, received, received
                ])?;
                touched.insert(id.clone());
            }
            Kind::Span {
                id,
                trace,
                parent,
                span_type,
                name,
                model,
                start,
                end,
                error,
                has_error,
                input_tokens,
                output_tokens,
            } => {
                let old: Option<String> = conn
                    .query_row("SELECT trace_id FROM spans WHERE id=?", [id], |r| r.get(0))
                    .optional()?;
                if let Some(old) = old {
                    touched.insert(old);
                }
                placeholder.execute(params![
                    trace,
                    serde_json::json!({"placeholder":true,"trace_id":trace}).to_string(),
                    received,
                    received
                ])?;
                span_insert.execute(params![
                    id,
                    trace,
                    parent,
                    span_type,
                    name,
                    model,
                    start,
                    end,
                    start.zip(*end).map(|(s, e)| (e - s).max(0)),
                    *has_error as i32,
                    error,
                    input_tokens,
                    output_tokens,
                    item.raw
                ])?;
                touched.insert(trace.clone());
            }
        }
    }
    Ok(())
}
pub fn refresh_bounds(conn: &Connection, touched: HashSet<String>) -> Result<()> {
    let mut bounds = conn.prepare_cached("UPDATE traces SET started_at=(SELECT MIN(started_at) FROM spans WHERE trace_id=?1),ended_at=(SELECT MAX(ended_at) FROM spans WHERE trace_id=?1) WHERE id=?1")?;
    let mut remove = conn.prepare_cached(
        "DELETE FROM trace_search WHERE rowid=(SELECT rowid FROM traces WHERE id=?)",
    )?;
    let mut insert = conn.prepare_cached("INSERT INTO trace_search(rowid,trace_id,workflow_name,group_id,models,errors) SELECT t.rowid,t.id,t.workflow_name,t.group_id,(SELECT GROUP_CONCAT(DISTINCT model) FROM spans WHERE trace_id=t.id),(SELECT GROUP_CONCAT(DISTINCT NULLIF(error_text,'')) FROM spans WHERE trace_id=t.id) FROM traces t WHERE t.id=?")?;
    for id in touched {
        bounds.execute([&id])?;
        remove.execute([&id])?;
        insert.execute([&id])?;
    }
    Ok(())
}
