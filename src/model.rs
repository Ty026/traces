use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Item {
    pub raw: String,
    pub kind: Kind,
}
#[derive(Clone, Debug)]
pub enum Kind {
    Trace {
        id: String,
        workflow: String,
        group: Option<String>,
        metadata: String,
    },
    Span {
        id: String,
        trace: String,
        parent: Option<String>,
        span_type: String,
        name: Option<String>,
        model: Option<String>,
        start: Option<i64>,
        end: Option<i64>,
        error: String,
        has_error: bool,
        input_tokens: i64,
        output_tokens: i64,
    },
}
fn string(v: &Value, key: &str) -> Option<String> {
    v.get(key)?
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
}
fn date(v: &Value, key: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(v.get(key)?.as_str()?)
        .ok()
        .map(|d| d.timestamp_millis())
}
impl Item {
    pub fn parse(v: &Value) -> Option<Self> {
        let id = string(v, "id")?;
        let kind = match v.get("object")?.as_str()? {
            "trace" => Kind::Trace {
                id,
                workflow: string(v, "workflow_name").unwrap_or_else(|| "Agent workflow".into()),
                group: string(v, "group_id"),
                metadata: v
                    .get("metadata")
                    .filter(|v| v.is_object())
                    .cloned()
                    .unwrap_or(serde_json::json!({}))
                    .to_string(),
            },
            "trace.span" => {
                let data = v.get("span_data").filter(|v| v.is_object())?;
                let span_type = string(data, "type").unwrap_or_else(|| "unknown".into());
                let model = string(data, "model");
                let name = string(data, "name").or_else(|| match span_type.as_str() {
                    "generation" => model.clone(),
                    "handoff" => {
                        let parts: Vec<_> = [string(data, "from_agent"), string(data, "to_agent")]
                            .into_iter()
                            .flatten()
                            .collect();
                        (!parts.is_empty()).then(|| parts.join(" → "))
                    }
                    "mcp_tools" => string(data, "server"),
                    _ => None,
                });
                let error = v.get("error").filter(|v| v.is_object());
                let usage = data.get("usage").unwrap_or(&Value::Null);
                Kind::Span {
                    id,
                    trace: string(v, "trace_id")?,
                    parent: string(v, "parent_id"),
                    span_type,
                    name,
                    model,
                    start: date(v, "started_at"),
                    end: date(v, "ended_at"),
                    error: error.map(ToString::to_string).unwrap_or_default(),
                    has_error: error.is_some(),
                    input_tokens: usage
                        .get("input_tokens")
                        .or_else(|| usage.get("prompt_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .max(0),
                    output_tokens: usage
                        .get("output_tokens")
                        .or_else(|| usage.get("completion_tokens"))
                        .and_then(Value::as_i64)
                        .unwrap_or(0)
                        .max(0),
                }
            }
            _ => return None,
        };
        Some(Self {
            raw: v.to_string(),
            kind,
        })
    }
    pub fn bytes(&self) -> usize {
        self.raw.len() * 3 + 1024
    }
}
