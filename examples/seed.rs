//! Synthetic data only. Refuses to seed a nonempty database; intended for isolated benchmarks.
use agent_traces::{
    auth,
    db::{Db, now},
    ingest::{persist_items, refresh_bounds},
    model::Item,
};
use anyhow::Result;
use serde_json::json;
use std::collections::HashSet;
#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let path = args
        .get(1)
        .expect("seed DATABASE [SPAN_COUNT] [PAYLOAD_BYTES]");
    let count: usize = args
        .get(2)
        .map(|s| s.parse())
        .transpose()?
        .unwrap_or(1000000);
    let bytes: usize = args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(4096);
    let db = Db::open(std::path::Path::new(path))?;
    let n = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM traces", [], |r| r.get::<_, i64>(0))?))
        .await?;
    anyhow::ensure!(n == 0, "Seed requires an empty database");
    let start = std::time::Instant::now();
    let received = now();
    for offset in (0..count).step_by(1000) {
        let end = (offset + 1000).min(count);
        db.write(move|c|{let tx=c.transaction()?;let mut touched=HashSet::new();let mut items=Vec::with_capacity(1100);
   for i in offset..end {
    let trace=format!("trace_{:08}",i/20);let slot=i%20;
    if slot==0{items.push(Item::parse(&json!({"object":"trace","id":trace,"workflow_name":(["Research assistant","Customer support","Document extraction","Code review","Travel planner"][i/20%5]),"group_id":format!("session_{}",i/200),"metadata":{"environment":"benchmark"}})).unwrap());}
    let mut v=json!({"object":"trace.span","id":format!("span_{i:09}"),"trace_id":trace,"parent_id":if slot==0{None}else{Some(format!("span_{:09}",i-slot))},"started_at":chrono::DateTime::from_timestamp_millis(received-5000+slot as i64*100).unwrap().to_rfc3339(),"ended_at":chrono::DateTime::from_timestamp_millis(received-4500+slot as i64*100).unwrap().to_rfc3339(),"span_data":{"type":if slot==0{"agent"}else if slot%3==0{"function"}else{"generation"},"name":if slot==0{"Research assistant"}else if slot%3==0{"search_documents"}else{"Generate response"},"model":if slot%3!=0{Some("model-a")}else{None},"input":[{"role":"user","content":format!("Summarize the latest findings for project {}.",i/20)}],"output":[{"role":"assistant","content":"The research suggests three practical next steps. Review the source material, compare the alternatives, and document the decision."}],"usage":{"input_tokens":420,"output_tokens":180}}});
    if i%100==19{v["error"]=json!({"message":"Upstream request timed out","code":"timeout"});}
    let length=v.to_string().len();if bytes>length{v["span_data"]["context"]=json!("x".repeat(bytes-length));}items.push(Item::parse(&v).unwrap());
   }persist_items(&tx,&items,received-(count-offset) as i64,&mut touched)?;refresh_bounds(&tx,touched)?;tx.commit()?;Ok(())}).await?;
        if end % 100000 == 0 {
            eprintln!(
                "Seeded {end} spans in {:.1}s",
                start.elapsed().as_secs_f64()
            );
        }
    }
    let user = auth::User {
        id: 1,
        login: "benchmark".into(),
        email: "benchmark@example.com".into(),
    };
    let session = auth::issue_session(&db, user).await?;
    let token = format!("tr_{}", auth::secret());
    let hash = auth::hash(&token);
    let prefix = token[..11].to_owned();
    db.write(move|c|{c.execute("INSERT INTO api_keys(id,name,prefix,hash,created_at) VALUES('benchmark','Benchmark',?,?,?)",rusqlite::params![prefix,hash,now()])?;c.execute_batch("ANALYZE; PRAGMA wal_checkpoint(TRUNCATE);")?;Ok(())}).await?;
    // Credentials are synthetic and restricted to this isolated database.
    println!(
        "{}",
        json!({"session":session,"key":token,"spans":count,"payloadBytes":bytes,"seedSeconds":start.elapsed().as_secs_f64()})
    );
    Ok(())
}
