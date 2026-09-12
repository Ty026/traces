use anyhow::Result;
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::{Connection, OptionalExtension, params, types::Value as Sql};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Default, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Filters {
    pub q: Option<String>,
    pub status: Option<String>,
    pub span_type: Option<String>,
    pub model: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub cursor: Option<String>,
    pub limit: Option<usize>,
}
fn escape(s: &str) -> String {
    s.replace('!', "!!").replace('%', "!%").replace('_', "!_")
}
pub fn list(conn: &Connection, f: &Filters) -> Result<Value> {
    let mut clauses = vec!["1=1".to_owned()];
    let mut args: Vec<Sql> = Vec::new();
    if let Some(q) = f.q.as_deref().filter(|q| !q.is_empty()) {
        if q.chars().count() >= 3 {
            clauses.push(
                "t.rowid IN (SELECT rowid FROM trace_search WHERE trace_search MATCH ?)".into(),
            );
            args.push(format!("\"{}\"", q.replace('"', "\"\"")).into());
        } else {
            clauses.push("t.rowid IN (SELECT rowid FROM trace_search WHERE trace_id LIKE ? ESCAPE '!' OR workflow_name LIKE ? ESCAPE '!' OR group_id LIKE ? ESCAPE '!' OR models LIKE ? ESCAPE '!' OR errors LIKE ? ESCAPE '!')".into());
            for _ in 0..5 {
                args.push(format!("%{}%", escape(q)).into());
            }
        }
    }
    match f.status.as_deref() {
        Some("errors") => clauses.push("t.error_count>0".into()),
        Some("open") => clauses.push("(t.unfinished_count>0 OR t.span_count=0)".into()),
        Some("no_errors") => clauses.push("t.error_count=0".into()),
        _ => {}
    }
    for (column, value) in [("span_type", &f.span_type), ("model", &f.model)] {
        if let Some(value) = value.as_ref().filter(|v| !v.is_empty() && *v != "all") {
            clauses.push(format!(
                "EXISTS(SELECT 1 FROM spans s WHERE s.trace_id=t.id AND s.{column}=?)"
            ));
            args.push(value.clone().into());
        }
    }
    if let Some(since) = f.since {
        clauses.push("t.last_seen>=?".into());
        args.push(since.into());
    }
    if let Some(until) = f.until {
        clauses.push("t.last_seen<=?".into());
        args.push(until.into());
    }
    if let Some(cursor) = &f.cursor {
        let (last, id): (i64, String) = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(cursor)?)?;
        clauses.push("(t.last_seen,t.id)<(?,?)".into());
        args.push(last.into());
        args.push(id.into());
    }
    let limit = f.limit.unwrap_or(50).clamp(1, 100);
    args.push(((limit + 1) as i64).into());
    let sql = format!(
        "SELECT t.id,t.workflow_name,t.group_id,t.first_seen,t.last_seen,t.span_count,t.error_count,t.unfinished_count,t.generation_count,t.function_count,t.started_at,t.ended_at FROM traces t WHERE {} ORDER BY t.last_seen DESC,t.id DESC LIMIT ?",
        clauses.join(" AND ")
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows: Vec<Value> = stmt
        .query_map(rusqlite::params_from_iter(args), summary)?
        .collect::<rusqlite::Result<_>>()?;
    let cursor = if rows.len() > limit {
        rows.pop();
        let row = &rows[limit - 1];
        Some(URL_SAFE_NO_PAD.encode(serde_json::to_vec(&(
            row["lastSeen"].as_i64().unwrap(),
            row["id"].as_str().unwrap(),
        ))?))
    } else {
        None
    };
    Ok(json!({"items": rows,"nextCursor":cursor}))
}
fn summary(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let start: Option<i64> = r.get(10)?;
    let end: Option<i64> = r.get(11)?;
    Ok(
        json!({"id":r.get::<_,String>(0)?,"workflowName":r.get::<_,String>(1)?,"groupId":r.get::<_,Option<String>>(2)?,"firstSeen":r.get::<_,i64>(3)?,"lastSeen":r.get::<_,i64>(4)?,"spanCount":r.get::<_,i64>(5)?,"errorCount":r.get::<_,i64>(6)?,"unfinishedCount":r.get::<_,i64>(7)?,"generationCount":r.get::<_,i64>(8)?,"functionCount":r.get::<_,i64>(9)?,"startedAt":start,"endedAt":end,"durationMs":start.zip(end).map(|(s,e)| (e-s).max(0))}),
    )
}
pub fn trace(conn: &Connection, id: &str) -> Result<Option<Value>> {
    let mut result = conn.query_row("SELECT id,workflow_name,group_id,first_seen,last_seen,span_count,error_count,unfinished_count,generation_count,function_count,started_at,ended_at FROM traces WHERE id=?", [id], summary).optional()?;
    if let Some(row) = &mut result {
        let (raw, metadata): (String, String) =
            conn.query_row("SELECT raw,metadata FROM traces WHERE id=?", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
        row["raw"] = serde_json::from_str(&raw)?;
        row["metadata"] = serde_json::from_str(&metadata)?;
    }
    Ok(result)
}
pub fn spans(conn: &Connection, id: &str) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT id,parent_id,span_type,name,model,started_at,ended_at,duration_ms,has_error,input_tokens,output_tokens FROM spans WHERE trace_id=? ORDER BY started_at,id")?;
    let rows: Vec<Value> = stmt.query_map([id], |r| Ok(json!({"id":r.get::<_,String>(0)?,"parentId":r.get::<_,Option<String>>(1)?,"spanType":r.get::<_,String>(2)?,"name":r.get::<_,Option<String>>(3)?,"model":r.get::<_,Option<String>>(4)?,"startedAt":r.get::<_,Option<i64>>(5)?,"endedAt":r.get::<_,Option<i64>>(6)?,"durationMs":r.get::<_,Option<i64>>(7)?,"hasError":r.get::<_,bool>(8)?,"inputTokens":r.get::<_,i64>(9)?,"outputTokens":r.get::<_,i64>(10)?})))?.collect::<rusqlite::Result<_>>()?;
    Ok(json!({"items":rows}))
}
pub fn span(conn: &Connection, trace: &str, id: &str) -> Result<Option<Value>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT raw FROM spans WHERE trace_id=? AND id=?",
            params![trace, id],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|raw| serde_json::from_str(&raw).map_err(Into::into))
        .transpose()
}
pub fn search(conn: &Connection, trace: &str, query: &str) -> Result<Value> {
    let mut stmt = conn.prepare("SELECT id FROM spans WHERE trace_id=? AND raw LIKE ? ESCAPE '!' ORDER BY started_at,id LIMIT 1001")?;
    let mut ids: Vec<String> = stmt
        .query_map(params![trace, format!("%{}%", escape(query))], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let truncated = ids.len() > 1000;
    ids.truncate(1000);
    Ok(json!({"ids":ids,"truncated":truncated}))
}
